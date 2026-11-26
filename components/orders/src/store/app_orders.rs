//! Order reads of the app platform: whether an order belongs to the shop, an imported order by idempotency key and orders carrying an app's configuration.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::{Result, State};

/// Whether the shop has the order.
pub async fn exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND id=$2) AS "owned!""#,
        t,
        id
    )
    .fetch_one(db)
    .await?)
}

/// An order imported before under the same idempotency key.
pub struct Imported {
    pub data: Value,
    pub fingerprint: String,
}

/// The order imported under an idempotency key, if any.
pub async fn imported(conn: &mut PgConnection, t: &str, key: &str) -> Result<Option<Imported>> {
    Ok(sqlx::query_as!(
        Imported,
        "SELECT data,fingerprint FROM orders WHERE tenant=$1 AND idempotency_key=$2",
        t,
        key
    )
    .fetch_optional(conn)
    .await?)
}

/// An order that carries a configuration of an app: its ID, number and line items.
pub struct Configured {
    pub id: String,
    pub number: Option<String>,
    pub items: Option<Value>,
}

/// Up to `limit` orders, newest first, whose lines contain either configuration shape, before the `after` cursor order.
pub async fn configured(
    a: &State,
    t: &str,
    (legacy, multiple): (&Value, &Value),
    after: &str,
    limit: i64,
) -> Result<Vec<Configured>> {
    Ok(sqlx::query_as!(
        Configured,
        "SELECT id,data->>'orderNumber' AS number,data->'cart'->'lineItems' AS items FROM orders WHERE tenant=$1 AND (data @> $2::jsonb OR data @> $3::jsonb) AND ($4::text='' OR (created_at,id)<(SELECT created_at,id FROM orders WHERE tenant=$1 AND id=$4)) ORDER BY created_at DESC,id DESC LIMIT $5",
        t,
        legacy,
        multiple,
        after,
        limit
    )
    .fetch_all(&a.db)
    .await?)
}
