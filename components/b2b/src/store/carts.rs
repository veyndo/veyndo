//! The cart rows procurement reads and changes: by token or id (optionally locked), the open revision, and the revisioned document write. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State, bad};
use veyndo_shop::types::cart::{StoredCart, upgrade_cart};

/// A full cart row before its document is typed.
struct CartRow {
    id: String,
    tenant: String,
    token: String,
    data: Value,
    revision: i64,
    status: String,
}

impl CartRow {
    /// Upgrades older stored shapes and types the document.
    fn stored(self) -> Result<StoredCart> {
        let mut data = self.data;
        upgrade_cart(&mut data);
        Ok(StoredCart {
            id: self.id,
            tenant: self.tenant,
            token: self.token,
            data: serde_json::from_value(data).map_err(|e| bad(e.to_string()))?,
            revision: self.revision,
            status: self.status,
        })
    }
}

/// The cart behind `token`, unlocked.
pub async fn by_token(a: &State, tenant: &str, token: &str) -> Result<Option<StoredCart>> {
    sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2",
        tenant,
        token
    )
    .fetch_optional(&a.db)
    .await?
    .map(CartRow::stored)
    .transpose()
}

/// The cart behind `token`, locked for this transaction.
pub async fn lock_by_token(
    conn: &mut PgConnection,
    tenant: &str,
    token: &str,
) -> Result<Option<StoredCart>> {
    sqlx::query_as!(CartRow, "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE", tenant, token)
        .fetch_optional(conn)
        .await?
        .map(CartRow::stored)
        .transpose()
}

/// Cart `id`, locked for this transaction.
pub async fn lock_by_id(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<StoredCart>> {
    sqlx::query_as!(CartRow, "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE", tenant, id)
        .fetch_optional(conn)
        .await?
        .map(CartRow::stored)
        .transpose()
}

/// The revision of open cart `id`, locked for update.
pub async fn open_revision(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT revision FROM carts WHERE tenant=$1 AND id=$2 AND status='open' FOR UPDATE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// The revision of open cart `id`, share-locked.
pub async fn open_revision_shared(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT revision FROM carts WHERE tenant=$1 AND id=$2 AND status='open' FOR SHARE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Stores the cart document at `revision`.
pub async fn save(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    data: &Value,
    revision: i64,
) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$3,revision=$4 WHERE tenant=$1 AND id=$2",
        tenant,
        id,
        data,
        revision
    )
    .execute(conn)
    .await?;
    Ok(())
}
