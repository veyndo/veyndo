//! Account-authorized funding choice, immutable consent and saved-instrument admission; cart possession is insufficient.
use crate::domain::attempt::Attempt;
use crate::ports::Buyers;
use crate::store::attempts;
use crate::store::funding;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, uid};
use veyndo_shop::types::cart::StoredCart;
/// The provider payment an order names.
#[derive(Deserialize, Default)]
#[serde(default)]
struct FundedOrder {
    #[serde(deserialize_with = "lenient")]
    payment: FundedPayment,
}

/// The attempt of an order's provider payment.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FundedPayment {
    attempt_id: Value,
}

pub async fn prepare(
    buyers: &dyn Buyers,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    h: &RequestContext,
    c: &StoredCart,
    o: &Value,
) -> Result<()> {
    let Some(choice) = c
        .data
        .checkout
        .as_ref()
        .and_then(|s| s.payment_funding.as_ref())
    else {
        return Ok(());
    };
    if !choice.save && choice.id.is_none() {
        return Ok(());
    }
    if choice.save && choice.id.is_some() {
        return Err(bad(
            "Choose an existing payment method or consent to saving a new one",
        ));
    }
    let token = header(h, "x-customer-token").ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Customer login required to save or use a payment method".into(),
    ))?;
    let customer = buyers.session_customer(tx, &c.tenant, &hash(token)).await?;
    let customer = customer
        .filter(|id| Some(id) == c.data.customer_id.as_ref())
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Payment method needs the current owning customer session".into(),
        ))?;
    let order: FundedOrder = view(o);
    let id = order
        .payment
        .attempt_id
        .as_str()
        .ok_or(bad("Saved payment requires a provider payment"))?;
    let p = attempts::get(&mut **tx, &c.tenant, id).await?;
    let ctx = p.ctx();
    if !ctx.can("vault") {
        return Err(bad(
            "This payment method does not support saved instruments",
        ));
    }
    let mut funding = json!({"customerId":customer,"save":choice.save,"consentId":uid(),"consentVersion":"save-payment@1"});
    if let Some(saved) = &choice.id {
        let r = funding::share_instrument(tx, &c.tenant, &customer, saved)
            .await?
            .ok_or(Error(
                StatusCode::NOT_FOUND,
                "Saved payment method unavailable".into(),
            ))?;
        if r.provider != p.provider
            || r.method != ctx.method
            || r.account_ref != ctx.account_ref
            || r.environment != p.environment
        {
            return Err(bad(
                "Saved payment method belongs to another payment account or method",
            ));
        }
        funding["reference"] = json!(r.reference);
        funding["instrumentId"] = json!(saved);
    }
    funding::bind(tx, &c.tenant, id, &funding).await?;
    Ok(())
}
/// A revoked instrument cannot start another queued provider operation. Already submitted financial effects still reconcile.
pub async fn dispatch_check(a: &State, p: &Attempt, op: &str) -> Result<()> {
    if op != "create" {
        return Ok(());
    }
    let funding = p.ctx().funding;
    if let Some(id) = funding.instrument_id.as_str() {
        let active = funding::instrument_active(
            &a.db,
            &p.tenant,
            id,
            funding.customer_id.as_str().unwrap_or(""),
        )
        .await?;
        if !active {
            return Err(conflict("Saved payment method has been revoked"));
        }
    }
    Ok(())
}
