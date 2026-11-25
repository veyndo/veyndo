//! Provider-neutral receipt admission and atomic allocation; verified evidence owns ledger transitions.
use crate::domain::attempt::{Attempt, context_of};
use crate::domain::receipt_ref::{ProviderReceipt, identity, is, reference};
use crate::domain::service_config::approval_url;
use crate::domain::{instructions, refunds, settlements, state};
use crate::ports::Host;
use crate::store::attempts::{Receipt, record_receipt};
use crate::store::jobs::enqueue_tx;
use crate::store::order_ledger::{release_stock, update_order};
use crate::store::receipt_claims::claim;
use crate::store::settlements as store_settlements;
use crate::store::{instructions as store_instructions, instruments, refund_receipts};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, StatusCode, bad, conflict};

/// The JSON fields `persist` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PersistInput {
    #[serde(deserialize_with = "lenient")]
    amount_minor: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    ledger_job_key: Option<String>,
    #[serde(deserialize_with = "lenient")]
    expiry: Option<bool>,
}

pub async fn persist(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    op: &str,
    input: &Value,
    v: &Value,
) -> Result<()> {
    let args: PersistInput = view(input);
    identity(p, v)?;
    let r: ProviderReceipt = view(v);
    if op == "capture" && r.capture_pending == Some(true) {
        return Err(Error(
            StatusCode::ACCEPTED,
            "Provider capture pending".into(),
        ));
    }
    let remote = reference(&r.reference)?;
    let status = r.status.as_deref().ok_or(bad("Payment status required"))?;
    let mut state = p.state.clone();
    let mut capture = p.capture.clone();
    let mut refunded = p.refunded;
    let mut context = p.context.clone();
    let instructions_changed = instructions::admit(p, &mut context, v)?;
    let url = approval_url(host, p, &r.approval_url)?;
    if op == "refund" {
        let amount = args
            .amount_minor
            .filter(|n| *n > 0)
            .ok_or(bad("Refund amount required"))?;
        if status == "refund_pending" {
            return Err(Error(
                StatusCode::ACCEPTED,
                "Provider refund pending".into(),
            ));
        }
        if status != "refunded"
            || r.refund.amount_minor != Some(amount)
            || !is(&r.refund.currency, &p.currency)
            || !r.confirmed()
        {
            return Err(bad("Refund receipt mismatch"));
        }
        let key = args
            .ledger_job_key
            .as_deref()
            .ok_or(bad("Refund ledger job required"))?;
        let delta = refund_receipts::apply(tx, p, &mut context, &r.refund.raw, Some(key)).await?;
        refunded = refunds::checked_total(settlements::captured(p), refunded, delta)?;
        state = settlements::state(
            p,
            settlements::captured(p),
            refunded,
            settlements::closed(p),
        )
        .into();
    } else if ["captured", "partially_captured"].contains(&status) {
        let (settled, id, closed) = store_settlements::admit(tx, p, &mut context, v).await?;
        capture = Some(id);
        state = settlements::state(p, settled, refunded, closed).into();
    } else if status == "authorized" {
        if !r.confirmed() || r.authorized_amount_minor != Some(p.amount) {
            return Err(bad("Authorization receipt mismatch"));
        }
        if !p.ctx().can("authorize") {
            return Err(bad("Method does not support authorization"));
        }
        let id = reference(&r.authorization_id)?;
        claim(tx, p, "authorization", id, "").await?;
        context["authorizationId"] = json!(id);
        if ["pending", "ready", "approved"].contains(&p.state.as_str()) {
            state = "authorized".into();
        }
    } else if ["cancelled", "voided"].contains(&status) {
        if !r.confirmed() || !["cancel", "void", "reconcile"].contains(&op) {
            return Err(bad("Cancellation requires verified provider confirmation"));
        }
        if p.state == "authorized" && status != "voided" {
            return Err(conflict("Void authorization before releasing stock"));
        }
        return release_stock(
            host,
            tx,
            p,
            if args.expiry == Some(true) {
                "expired"
            } else {
                "cancelled"
            },
            status == "voided" && r.confirmed(),
        )
        .await;
    } else if ["pending", "ready", "approved"].contains(&status) {
        if status == "approved" && !r.confirmed() {
            return Err(bad("Approval must be server verified"));
        }
        if ["pending", "ready", "approved"].contains(&p.state.as_str()) {
            let ranks = ["pending", "ready", "approved"];
            if ranks.iter().position(|v| *v == status) >= ranks.iter().position(|v| *v == p.state) {
                state = status.into();
            }
        }
    } else {
        return Err(bad("Unsupported provider receipt state"));
    }
    state::transition(&p.state, &state)?;
    if ["captured", "partially_captured"].contains(&status) && r.confirmed() {
        refunded = refund_receipts::reconcile(tx, p, &mut context, v).await?;
        if refunded > 0 {
            let settled = context_of(&context);
            let next = settlements::state(
                p,
                settled.captured_amount_minor.as_i64().unwrap_or(p.amount),
                refunded,
                settled.capture_closed == true,
            );
            state::transition(&state, next)?;
            state = next.into();
        }
    }
    record_receipt(
        tx,
        p,
        Receipt {
            state: &state,
            provider_order: remote,
            approval_url: url.as_deref(),
            capture: capture.as_deref(),
            refunded,
            context: &context,
        },
    )
    .await?;
    if state != p.state
        || capture != p.capture
        || refunded != p.refunded
        || context_of(&context).captured_amount_minor != p.ctx().captured_amount_minor
        || context_of(&context).capture_closed != p.ctx().capture_closed
    {
        follow(host, tx, p, &state).await?;
    }
    instruments::observe(tx, p, v).await?;
    if instructions_changed {
        store_instructions::publish(tx, p, &context).await?;
    }
    Ok(())
}

/// A changed ledger state queues the capture or authorization an approval asks for and moves the order along.
async fn follow(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
) -> Result<()> {
    let intent = p.ctx().intent;
    if state == "approved" || (state == "authorized" && intent == "capture") {
        let op = if intent == "authorize" {
            "authorize"
        } else {
            "capture"
        };
        enqueue_tx(
            tx,
            &p.tenant,
            &p.id,
            op,
            &format!("approved:{}", p.id),
            &json!({}),
        )
        .await?;
    }
    update_order(host, tx, p, state).await
}

#[cfg(test)]
mod tests {

    use crate::domain::attempt::Attempt;
    use crate::domain::receipt_ref::{identity, reference};

    use serde_json::json;

    #[test]
    fn every_receipt_is_bound_to_its_original_account_and_invoice() {
        let p = Attempt {
            id: "a".into(),
            provider: "example".into(),
            context: json!({"accountRef":"merchant"}),
            tenant: "shop".into(),
            order: "o".into(),
            amount: 123,
            currency: "EUR".into(),
            currency_scale: 2,
            state: "ready".into(),
            provider_order: Some("remote".into()),
            capture: None,
            refunded: 0,
            adapter_version: "1.0.0".into(),
            environment: "sandbox".into(),
            bn_code: "".into(),
        };
        let v = json!({"apiVersion":"1","provider":"example","adapterVersion":"1.0.0","tenant":"shop","attemptId":"a","orderId":"o","environment":"sandbox","accountRef":"merchant","currency":"EUR","amountMinor":123,"reference":"remote"});
        assert!(identity(&p, &v).is_ok());
        for key in [
            "apiVersion",
            "provider",
            "adapterVersion",
            "tenant",
            "attemptId",
            "orderId",
            "environment",
            "accountRef",
            "currency",
            "amountMinor",
            "reference",
        ] {
            let mut wrong = v.clone();
            wrong[key] = json!("foreign");
            assert!(identity(&p, &wrong).is_err(), "{key}");
        }
        assert!(reference(&json!("../unsafe")).is_err());
    }
}
