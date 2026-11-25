//! The downloadable files of paid orders: a customer's list, and one file for its customer or its guest cart. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A file of one of the customer's orders.
pub struct OwnedDownload {
    pub id: String,
    pub title: Value,
    pub filename: String,
    pub order_id: String,
    /// The order, which decides whether its payment unlocks the file.
    pub data: Value,
}

/// The files of the customer's orders, newest order first (at most 200).
pub async fn owned(a: &State, tenant: &str, email: &str) -> Result<Vec<OwnedDownload>> {
    Ok(sqlx::query_as!(
        OwnedDownload,
        r#"SELECT a.id,a.title,a.filename,o.id AS order_id,o.data FROM order_downloads d JOIN product_assets a ON a.tenant=d.tenant AND a.id=d.asset_id JOIN orders o ON o.tenant=d.tenant AND o.id=d.order_id WHERE d.tenant=$1 AND (o.data->'orderCustomer'->>'customerId')=(SELECT id FROM customers WHERE tenant=o.tenant AND email=$2) ORDER BY o.created_at DESC LIMIT 200"#,
        tenant,
        email
    )
    .fetch_all(&a.db)
    .await?)
}

/// One file with the order it belongs to.
pub struct DownloadFile {
    pub mime: String,
    pub filename: String,
    pub content: Vec<u8>,
    pub data: Value,
}

/// One file of an order, for the signed-in customer (`email`) or the order's own guest cart (`cart_token`).
pub async fn file(
    a: &State,
    tenant: &str,
    email: Option<&str>,
    order: &str,
    asset: &str,
    cart_token: &str,
) -> Result<Option<DownloadFile>> {
    Ok(sqlx::query_as!(
        DownloadFile,
        r#"SELECT a.mime,a.filename,a.content,o.data FROM order_downloads d JOIN product_assets a ON a.tenant=d.tenant AND a.id=d.asset_id JOIN orders o ON o.tenant=d.tenant AND o.id=d.order_id JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE d.tenant=$1 AND d.order_id=$3 AND d.asset_id=$4 AND (($2::text IS NOT NULL AND (o.data->'orderCustomer'->>'customerId')=(SELECT id FROM customers WHERE tenant=o.tenant AND email=$2)) OR (length($5)>0 AND c.token=$5 AND c.data->>'admin_actor' IS NULL AND c.data->>'pos_actor' IS NULL))"#,
        tenant,
        email,
        order,
        asset,
        cart_token
    )
    .fetch_optional(&a.db)
    .await?)
}
