//! The cart rows company procurement reads and changes inside its own transactions: by token (unlocked) or id (locked), the open revision and the revisioned document write. The B2B component reaches them through its records port. Columns checked at compile time.
use crate::store::carts::CartRow;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The cart behind `token`, unlocked.
pub async fn by_token(a: &State, tenant: &str, token: &str) -> Result<Option<CartRow>> {
    Ok(sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2",
        tenant,
        token
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Cart `id`, locked for this transaction.
pub async fn lock_by_id(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<CartRow>> {
    Ok(sqlx::query_as!(CartRow, "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE", tenant, id)
        .fetch_optional(conn)
        .await?)
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
