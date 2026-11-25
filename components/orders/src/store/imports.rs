//! Historical order imports: the imported order state, idempotent replays and the insert of an order with its completed cart and import activity.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A historical order with its completed cart and import activity.
pub struct HistoricalOrder<'a> {
    pub id: &'a str,
    pub cart_id: &'a str,
    pub token: String,
    pub cart: Value,
    pub key: &'a str,
    pub fingerprint: &'a str,
    pub order: &'a Value,
    pub created: String,
    pub actor: &'a str,
    pub activity: Value,
}

/// An order's data with its UTC creation time as `sourceCreatedAt`.
pub async fn order(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT data||jsonb_build_object('sourceCreatedAt',to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) AS "value!" FROM orders WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// The fingerprint of an order already imported under the idempotency key.
pub async fn imported_fingerprint(
    c: &mut sqlx::PgConnection,
    t: &str,
    key: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query!(
        r#"SELECT data,fingerprint AS "fingerprint!" FROM orders WHERE tenant=$1 AND idempotency_key=$2"#,
        t,
        key
    )
    .fetch_optional(&mut *c)
    .await?
    .map(|old| old.fingerprint))
}

/// Inserts a historical order, its completed cart and its import activity.
pub async fn insert_historical(
    c: &mut sqlx::PgConnection,
    t: &str,
    o: HistoricalOrder<'_>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO carts(id,tenant,token,data,status) VALUES($1,$2,$3,$4,'completed')",
        o.cart_id,
        t,
        o.token,
        o.cart
    )
    .execute(&mut *c)
    .await?;
    sqlx::query!("INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data,created_at) VALUES($1,$2,$3,$4,$5,$6,$7::text::timestamptz)", o.id, t, o.cart_id, o.key, o.fingerprint, o.order, o.created).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'historical_import',$4)", t, o.id, o.actor, o.activity).execute(&mut *c).await?;
    Ok(())
}
