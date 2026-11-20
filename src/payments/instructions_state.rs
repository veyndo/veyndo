//! Immutable recipient evidence in the existing payment context, native invoice snapshots and one scoped outbox event.
use super::attempt;
use crate::{App, RequestContext, Result, Value, auth, bad, conflict, merchant};
use veyndo_payments::domain::instructions::snapshot;
/// The issuer already holds the order lock and document/PII authority. This immutable read takes no reverse payment lock and performs no provider I/O.
pub(crate) async fn for_document(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    order: &Value,
) -> Result<Option<Value>> {
    let Some(id) = order["payment"]["attemptId"].as_str() else {
        return Ok(None);
    };
    let row =
        sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2 AND order_id=$3")
            .bind(t)
            .bind(id)
            .bind(order["id"].as_str().unwrap_or(""))
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(bad("Document payment is not bound to this order"))?;
    let p = attempt(&row);
    if !p.context["capabilities"]
        .as_array()
        .is_some_and(|caps| caps.iter().any(|cap| cap == "instructions"))
    {
        return Ok(None);
    }
    let value=p.context.get("recipientInstructions").ok_or(conflict("Invoice payment instructions are still pending; check the payment before issuing the invoice"))?;
    snapshot(&p, value).map(Some)
}
/// Flow actions recheck current compound rights and fetch the original order's cached evidence at execution time.
pub(crate) async fn for_flow(a: &App, h: &RequestContext, order: &str) -> Result<Value> {
    let t = merchant(a, h)?;
    for scope in ["payments.read", "orders.read", "customers.pii"] {
        auth::permit(h, scope)?;
    }
    let row=sqlx::query("SELECT p.* FROM payment_attempts p JOIN orders o ON o.tenant=p.tenant AND o.id=p.order_id AND o.data#>>'{payment,attemptId}'=p.id WHERE p.tenant=$1 AND o.id=$2").bind(t).bind(order).fetch_one(&a.db).await?;
    let p = attempt(&row);
    snapshot(
        &p,
        p.context
            .get("recipientInstructions")
            .ok_or(bad("Payment instructions unavailable"))?,
    )
}
