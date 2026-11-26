//! The cart rows coupon selection changes: the token-locked read and the revision bump. The rows belong to checkout and are reached through the records port on the caller's connection.
use crate::ports::records;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Error, Result, StatusCode};
use veyndo_shop::types::cart::StoredCart;

/// The cart behind `token`, locked for this transaction; an unknown or expired
/// token is a 404 like the cart read, not a database failure.
pub async fn lock(conn: &mut PgConnection, tenant: &str, token: &str) -> Result<StoredCart> {
    records()?
        .lock_cart(conn, tenant, token)
        .await?
        .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Cart not found".into()))
}

/// Stores the cart document and advances its revision by one.
pub async fn save_next_revision(
    conn: &mut PgConnection,
    data: &Value,
    id: &str,
    tenant: &str,
) -> Result<()> {
    records()?
        .save_cart_next_revision(conn, data, (tenant, id))
        .await
}
