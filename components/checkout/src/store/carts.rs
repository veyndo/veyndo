//! Locked cart reads and token rotation of the checkout transaction, hydrated as the shared `StoredCart` contract. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, bad};
use veyndo_shop::types::cart::{StoredCart, upgrade_cart};

/// A complete cart row before its document is hydrated.
pub struct CartRow {
    pub id: String,
    pub tenant: String,
    pub token: String,
    pub data: Value,
    pub revision: i64,
    pub status: String,
}

impl CartRow {
    /// The shared cart contract; older stored shapes are upgraded on the way, as `veyndo_shop::types::cart::stored` does.
    pub fn into_cart(self) -> Result<StoredCart> {
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

/// The request's cart by its token, row-locked for the rest of the transaction.
pub async fn lock_by_token(
    tx: &mut PgConnection,
    tenant: &str,
    token: &str,
) -> Result<Option<CartRow>> {
    Ok(sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE",
        tenant,
        token
    )
    .fetch_optional(tx)
    .await?)
}

/// Gives the cart a fresh token, fencing every holder of the old one.
pub async fn rotate_token(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    token: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET token=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
        token,
        tenant,
        id
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Gives the cart a fresh token and returns the updated row.
pub async fn rotate_token_returning(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    token: &str,
) -> Result<CartRow> {
    Ok(sqlx::query_as!(
        CartRow,
        "UPDATE carts SET token=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 RETURNING id,tenant,token,data,revision,status",
        token,
        tenant,
        id
    )
    .fetch_one(tx)
    .await?)
}

/// Cancels an open cart; the number of carts changed (0 when it is no longer open).
pub async fn cancel_open(a: &veyndo_core::State, id: &str) -> veyndo_core::Result<u64> {
    Ok(sqlx::query!(
        "UPDATE carts SET status='cancelled',revision=revision+1 WHERE id=$1 AND status='open'",
        id
    )
    .execute(&a.db)
    .await?
    .rows_affected())
}
