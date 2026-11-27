//! The order follows its payment: state, settlement summary, events and the release of reserved stock in one transaction.
use crate::domain::attempt::Attempt;
use crate::domain::{settlements, state};
use crate::ports::Host;
use crate::store::attempts;
use crate::types::events::AttemptRecorded;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::document_slot::field;
use veyndo_core::{Result, conflict};
use veyndo_inventory::store::attempt_reservations::release_attempt;
use veyndo_orders::domain::order_fields::order_fields;
use veyndo_orders::domain::order_patches::{Revised, Stated, strip_cart_token};
use veyndo_pricing::verified_kernel;

/// The order document of a payment (a missing row is a database error).
pub async fn order_document<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    tenant: &str,
    order: &str,
) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2",
        tenant,
        order
    )
    .fetch_one(db)
    .await?)
}

/// The order fields the ledger reads back while updating the payment state.
#[derive(Deserialize, Default)]
#[serde(default)]
struct LedgerOrder {
    revision: Value,
    #[serde(deserialize_with = "lenient")]
    cart: LedgerCart,
}

/// The cart an order was placed from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct LedgerCart {
    id: Value,
}

/// Whether live money was charged for the order's payment.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RealMoney {
    real_money_charged: bool,
}

pub async fn update_order(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
) -> Result<()> {
    let mut o: Value = sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE",
        p.tenant,
        p.order
    )
    .fetch_one(&mut **tx)
    .await?;
    merge(field(&mut o, "payment")?, Stated { state })?;
    let current = attempts::get(&mut **tx, &p.tenant, &p.id).await?;
    let summary = settlements::summary(&current);
    let payment = field(&mut o, "payment")?;
    if let (Some(payment), Some(fields)) = (payment.as_object_mut(), summary.as_object()) {
        payment.extend(fields.clone());
    }
    merge(
        payment,
        RealMoney {
            real_money_charged: p.environment == "live"
                && [
                    "captured",
                    "partially_captured",
                    "captured_late",
                    "partially_refunded",
                    "refunded",
                ]
                .contains(&state),
        },
    )?;
    let revision = view::<LedgerOrder>(&o).revision.as_i64().unwrap_or(1) + 1;
    merge(&mut o, Revised { revision })?;
    if state == "captured_late" {
        merge(
            &mut o,
            Stated {
                state: "payment_review",
            },
        )?;
    }
    if ["cancelled", "expired"].contains(&state) {
        merge(&mut o, Stated { state })?;
    }
    host.captured(tx, &p.tenant, &mut o, state).await?;
    order_fields(&mut o);
    let cart = view::<LedgerOrder>(&o).cart.id;
    let cart = cart.as_str().unwrap();
    sqlx::query!(
        "UPDATE orders SET data=$1 WHERE tenant=$2 AND id=$3",
        o,
        p.tenant,
        p.order
    )
    .execute(&mut **tx)
    .await?;
    host.mirror_cart_order(tx, &p.tenant, cart, &o).await?;
    let mut event_order = o.clone();
    strip_cart_token(&mut event_order)?;
    let kind = if state == "captured" && p.state != "captured" {
        "payment.captured"
    } else {
        "payment.updated"
    };
    let recorded = AttemptRecorded {
        order_id: &p.order,
        attempt_id: &p.id,
        state,
        provider: &p.provider,
        environment: &p.environment,
        real_money_charged: p.environment == "live" && state == "captured",
        order: event_order,
    };
    veyndo_core::events::emit_as(&mut **tx, &p.tenant, kind, &recorded).await?;
    if state == "captured" {
        host.reward_capture(tx, &p.tenant, cart).await?;
    }
    Ok(())
}
pub async fn release_stock(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
    void_confirmed: bool,
) -> Result<()> {
    if !verified_kernel::reservation_release_admissible(
        ["pending", "ready", "approved"].contains(&p.state.as_str()),
        p.state == "authorized",
        void_confirmed,
    ) {
        return Err(conflict(
            "Captured or uncertain payment cannot release inventory",
        ));
    }
    state::transition(&p.state, state)?;
    veyndo_inventory::store::reservations::release(tx, &p.tenant, &p.order).await?;
    release_attempt(tx, &p.tenant, &p.id).await?;
    sqlx::query!(
        "UPDATE payment_attempts SET state=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
        state,
        p.tenant,
        p.id
    )
    .execute(&mut **tx)
    .await?;
    update_order(host, tx, p, state).await
}

#[cfg(test)]
mod tests {
    use super::RealMoney;
    use serde_json::json;
    use veyndo_core::document::merge;
    use veyndo_core::document_slot::field;
    use veyndo_orders::domain::order_patches::{Revised, Stated};

    /// The typed ledger patches write exactly the order document the indexed assignments wrote.
    #[test]
    fn typed_ledger_patches_match_the_indexed_writes() {
        let summary = json!({"capturedMinor":1000,"refundedMinor":0});
        for state in ["captured", "captured_late", "cancelled", "pending"] {
            let base = json!({"id":"o1","state":"placed","revision":3,"payment":{"state":"pending","attemptId":"a1"}});
            let mut indexed = base.clone();
            indexed["payment"]["state"] = json!(state);
            if let (Some(payment), Some(fields)) =
                (indexed["payment"].as_object_mut(), summary.as_object())
            {
                payment.extend(fields.clone());
            }
            indexed["payment"]["realMoneyCharged"] = json!(state == "captured");
            indexed["revision"] = json!(4);
            if state == "captured_late" {
                indexed["state"] = json!("payment_review");
            }
            if state == "cancelled" {
                indexed["state"] = json!(state);
            }
            let mut typed = base.clone();
            merge(field(&mut typed, "payment").unwrap(), Stated { state }).unwrap();
            let payment = field(&mut typed, "payment").unwrap();
            if let (Some(payment), Some(fields)) = (payment.as_object_mut(), summary.as_object()) {
                payment.extend(fields.clone());
            }
            merge(
                payment,
                RealMoney {
                    real_money_charged: state == "captured",
                },
            )
            .unwrap();
            merge(&mut typed, Revised { revision: 4_i64 }).unwrap();
            if state == "captured_late" {
                merge(
                    &mut typed,
                    Stated {
                        state: "payment_review",
                    },
                )
                .unwrap();
            }
            if state == "cancelled" {
                merge(&mut typed, Stated { state }).unwrap();
            }
            assert_eq!(typed, indexed, "{state}");
        }
    }
}
