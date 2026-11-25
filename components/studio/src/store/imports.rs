//! SQL of reviewed migration imports: idempotent replays of already imported records, the native state inspections compare against and historical order inserts.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::{Result, State};

/// A category's data document.
pub async fn category_data(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM categories WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// A category as `{parentId,data}`.
pub async fn category(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('parentId',parent_id,'data',data) AS "value!" FROM categories WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// A promotion's data document.
pub async fn promotion(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_promotions WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// A product's `extra` document.
pub async fn product_extra(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT extra FROM products WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// A product's media list.
pub async fn product_media(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT media FROM products WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// An asset's metadata with the digest and length of its content.
pub async fn asset(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('productId',product_id,'digest',encode(sha256(content),'hex'),'byteLength',octet_length(content),'mime',mime,'filename',filename,'title',title,'public',public) AS "value!" FROM product_assets WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// The imported fields of a customer.
pub struct Customer {
    pub email: String,
    pub profile: Value,
    pub group: String,
    pub automation: Value,
}

/// A customer's imported fields.
pub async fn customer(a: &State, t: &str, id: &str) -> Result<Option<Customer>> {
    Ok(sqlx::query_as!(
        Customer,
        r#"SELECT email,profile,group_name AS "group",automation FROM customers WHERE tenant=$1 AND id=$2"#,
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// An order's data with its UTC creation time as `sourceCreatedAt`.
pub async fn order(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT data||jsonb_build_object('sourceCreatedAt',to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) AS "value!" FROM orders WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// A review in its import shape.
pub async fn review(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('productId',product_id,'sourceAuthor',author,'rating',rating,'title',title,'text',content,'verifiedPurchase',verified,'sourceDigest',source_import->>'recordDigest','createdAt',to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) AS "value!" FROM product_reviews WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// Serializes imports of one historical source identity (`<tenant>:<key>`).
pub async fn lock_historical(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,28))", key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The fingerprint of an order already imported under the idempotency key.
pub async fn imported_fingerprint(tx: &mut Tx<'_>, t: &str, key: &str) -> Result<Option<String>> {
    Ok(sqlx::query!(
        r#"SELECT data,fingerprint AS "fingerprint!" FROM orders WHERE tenant=$1 AND idempotency_key=$2"#,
        t,
        key
    )
    .fetch_optional(&mut **tx)
    .await?
    .map(|old| old.fingerprint))
}

/// Share-locks a sales channel; `false` when it does not exist.
pub async fn share_channel(tx: &mut Tx<'_>, t: &str, channel: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
        t,
        channel
    )
    .fetch_optional(&mut **tx)
    .await?
    .is_some())
}

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

/// Inserts a historical order, its completed cart and its import activity.
pub async fn insert_historical(tx: &mut Tx<'_>, t: &str, o: HistoricalOrder<'_>) -> Result<()> {
    sqlx::query!(
        "INSERT INTO carts(id,tenant,token,data,status) VALUES($1,$2,$3,$4,'completed')",
        o.cart_id,
        t,
        o.token,
        o.cart
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!("INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data,created_at) VALUES($1,$2,$3,$4,$5,$6,$7::text::timestamptz)", o.id, t, o.cart_id, o.key, o.fingerprint, o.order, o.created).execute(&mut **tx).await?;
    sqlx::query!("INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'historical_import',$4)", t, o.id, o.actor, o.activity).execute(&mut **tx).await?;
    Ok(())
}
