//! Shared operational order writes: one replay ledger, immutable quote, audit, cart projection and outbox.
use crate::domain::order_fields::order_fields;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad, conflict};
pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

pub async fn replay(
    tx: &mut Tx<'_>,
    t: &str,
    key: &str,
    fingerprint: &str,
) -> Result<Option<Value>> {
    if !(8..=128).contains(&key.len()) {
        return Err(bad("Invalid order request key"));
    }
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,28))",
        format!("{t}:{key}")
    )
    .execute(&mut **tx)
    .await?;
    let r = sqlx::query!("SELECT fingerprint,response FROM order_transition_requests WHERE tenant=$1 AND request_key=$2", t, key).fetch_optional(&mut **tx).await?;
    if let Some(r) = r {
        if r.fingerprint != fingerprint {
            return Err(conflict("Order request key reused with different input"));
        }
        return Ok(Some(r.response));
    }
    Ok(None)
}

pub struct Audit<'a> {
    pub kind: &'a str,
    pub data: Value,
    pub event_kind: &'a str,
    pub event: Value,
}

/// The change kind and target state an audit entry records.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AuditFields {
    kind: Value,
    state: Value,
}

/// The identity, revision and import origin of an order being persisted.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MutatedOrder {
    historical_import: Value,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    revision: Value,
}
pub async fn persist(
    tx: &mut Tx<'_>,
    t: &str,
    h: &RequestContext,
    o: &mut Value,
    audit: Audit<'_>,
) -> Result<()> {
    let stored: MutatedOrder = view(o);
    if !stored.historical_import.is_null() {
        return Err(conflict(
            "Historical order snapshots cannot execute new operational transitions",
        ));
    }
    order_fields(o);
    let id = stored
        .id
        .as_deref()
        .ok_or(bad("Order identity unavailable"))?;
    sqlx::query!(
        "INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,$4,$5)",
        t,
        id,
        h.principal.user.as_deref().unwrap_or("unknown"),
        audit.kind,
        audit.data
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        "UPDATE orders SET data=$1 WHERE tenant=$2 AND id=$3",
        &*o,
        t,
        id
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!("UPDATE carts SET data=jsonb_set(carts.data,'{order}',$1) FROM orders o WHERE carts.tenant=$2 AND carts.id=o.cart_id AND o.tenant=$2 AND o.id=$3", &*o, t, id).execute(&mut **tx).await?;
    veyndo_core::store::outbox::publish_on(&mut **tx, t, "order.updated", &json!({"orderId":id,"kind":view::<AuditFields>(&audit.data).kind,"state":view::<AuditFields>(&audit.data).state,"revision":stored.revision})).await?;
    veyndo_core::store::outbox::publish_on(&mut **tx, t, audit.event_kind, &audit.event).await?;
    Ok(())
}

pub async fn remember(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    key: &str,
    fingerprint: &str,
    response: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO order_transition_requests(tenant,request_key,order_id,fingerprint,response) VALUES($1,$2,$3,$4,$5)", t, key, id, fingerprint, response).execute(&mut **tx).await?;
    Ok(())
}

/// The order's document, locked for this transaction; `None` when the shop has no such order.
pub async fn lock_data(
    conn: &mut sqlx::PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}
