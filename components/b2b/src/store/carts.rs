//! The cart rows procurement reads and changes: by token or id (optionally locked), the open revision and the revisioned document write. The rows belong to checkout and are reached through the records port on the caller's connection.
use crate::ports::records;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};
use veyndo_shop::types::cart::StoredCart;

/// The cart behind `token`, unlocked.
pub async fn by_token(a: &State, tenant: &str, token: &str) -> Result<Option<StoredCart>> {
    records()?.cart_by_token(a, tenant, token).await
}

/// The cart behind `token`, locked for this transaction.
pub async fn lock_by_token(
    conn: &mut PgConnection,
    tenant: &str,
    token: &str,
) -> Result<Option<StoredCart>> {
    records()?.lock_cart_by_token(conn, tenant, token).await
}

/// Cart `id`, locked for this transaction.
pub async fn lock_by_id(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<StoredCart>> {
    records()?.lock_cart(conn, tenant, id).await
}

/// The revision of open cart `id`, locked for update.
pub async fn open_revision(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<i64>> {
    records()?.open_cart_revision(conn, tenant, id, false).await
}

/// The revision of open cart `id`, share-locked.
pub async fn open_revision_shared(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<i64>> {
    records()?.open_cart_revision(conn, tenant, id, true).await
}

/// Stores the cart document at `revision`.
pub async fn save(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    data: &Value,
    revision: i64,
) -> Result<()> {
    records()?
        .save_cart(conn, (tenant, id), data, revision)
        .await
}
