//! Storage of the external order import: replay by idempotency key, the locked product lines and their labels, and the imported cart and order rows.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

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

/// The distinct parents of the given products.
pub async fn parents(conn: &mut PgConnection, t: &str, ids: &[String]) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT parent_id AS "parent_id!" FROM products WHERE tenant=$1 AND id=ANY($2) AND parent_id IS NOT NULL"#,
        t,
        ids
    )
    .fetch_all(conn)
    .await?)
}

/// The product columns an imported order line reads.
pub struct LineProduct {
    pub id: String,
    pub name: String,
    pub stock: i32,
    pub parent_id: Option<String>,
    pub active: bool,
    pub product_number: Option<String>,
}

/// Locks the given products in ID order and returns the columns the order lines read.
pub async fn lock_products(
    conn: &mut PgConnection,
    t: &str,
    ids: &[String],
) -> Result<Vec<LineProduct>> {
    Ok(sqlx::query_as!(
        LineProduct,
        "SELECT id,name,stock,parent_id,active,product_number FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE",
        t,
        ids
    )
    .fetch_all(conn)
    .await?)
}

/// Whether one of the products is hidden in the sales channel.
pub async fn hidden_in_channel(
    conn: &mut PgConnection,
    t: &str,
    channel: &str,
    ids: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM product_channel_visibility WHERE tenant=$1 AND channel_id=$2 AND product_id=ANY($3) AND NOT visible) AS "hidden!""#,
        t,
        channel,
        ids
    )
    .fetch_one(conn)
    .await?)
}

/// A translated product name for an imported line.
pub struct Label {
    pub id: String,
    pub name: String,
}

/// The first non-blank translated name per product along the locale chain, the product's own before its parent's.
pub async fn labels(
    conn: &mut PgConnection,
    t: &str,
    ids: &[String],
    chain: &[String],
) -> Result<Vec<Label>> {
    Ok(
        sqlx::query_file_as!(Label, "src/store/external_orders/labels.sql", t, ids, chain)
            .fetch_all(conn)
            .await?,
    )
}

/// The sales channel document, share-locked for the import.
pub async fn lock_channel(conn: &mut PgConnection, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
        t,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Stores the completed cart that backs an imported order.
pub async fn insert_cart(
    conn: &mut PgConnection,
    id: &str,
    t: &str,
    token: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO carts(id,tenant,token,data,status) VALUES($1,$2,$3,$4,'completed')",
        id,
        t,
        token,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The imported order row.
pub struct NewOrder<'a> {
    pub id: &'a str,
    pub tenant: &'a str,
    pub cart_id: &'a str,
    pub idempotency_key: &'a str,
    pub fingerprint: &'a str,
    pub data: &'a Value,
}

/// Stores an imported order.
pub async fn insert_order(conn: &mut PgConnection, o: NewOrder<'_>) -> Result<()> {
    sqlx::query!(
        "INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data) VALUES($1,$2,$3,$4,$5,$6)",
        o.id,
        o.tenant,
        o.cart_id,
        o.idempotency_key,
        o.fingerprint,
        o.data
    )
    .execute(conn)
    .await?;
    Ok(())
}
