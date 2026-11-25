//! Single-use hosted checkout tickets and their recovery keys; every read and write runs under the caller's cart lock. Columns checked at compile time.
use crate::store::carts::CartRow;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Forgets the shop's tickets whose ticket and recovery windows both ended.
pub async fn purge_expired(tx: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query!("DELETE FROM checkout_handoffs WHERE tenant=$1 AND greatest(expires_at,recovery_expires_at)<now()", tenant)
        .execute(tx)
        .await?;
    Ok(())
}

/// A new ticket of a cart.
pub struct Ticket<'a> {
    pub digest: &'a str,
    pub tenant: &'a str,
    pub cart: &'a str,
    /// The recovery key digest of a framing storefront, valid for 24 hours.
    pub recovery_digest: Option<&'a str>,
    pub recovery_origin: Option<&'a str>,
}

/// Issues the cart's ten-minute ticket, replacing any earlier one.
pub async fn issue(tx: &mut PgConnection, t: &Ticket<'_>) -> Result<()> {
    sqlx::query!("INSERT INTO checkout_handoffs(digest,tenant,cart_id,expires_at,recovery_digest,recovery_origin,recovery_expires_at) VALUES($1,$2,$3,now()+interval '10 minutes',$4::text,$5,CASE WHEN $4::text IS NOT NULL THEN now()+interval '24 hours' END) ON CONFLICT(cart_id) DO UPDATE SET digest=EXCLUDED.digest,expires_at=EXCLUDED.expires_at,recovery_digest=EXCLUDED.recovery_digest,recovery_origin=EXCLUDED.recovery_origin,recovery_expires_at=EXCLUDED.recovery_expires_at,consumed_at=NULL", t.digest, t.tenant, t.cart, t.recovery_digest, t.recovery_origin)
        .execute(tx)
        .await?;
    Ok(())
}

/// A recovery request: the cart, its recovery key digest and the framing origin.
pub struct RecoveryKey<'a> {
    pub tenant: &'a str,
    pub cart: &'a str,
    pub digest: &'a str,
    pub origin: &'a str,
}

/// The cart of a valid recovery key; `lock` row-locks it, otherwise the last committed state is read.
pub async fn recovered_cart(
    tx: &mut PgConnection,
    k: &RecoveryKey<'_>,
    lock: bool,
) -> Result<Option<CartRow>> {
    Ok(if lock {
        sqlx::query_as!(
            CartRow,
            "SELECT c.id,c.tenant,c.token,c.data,c.revision,c.status FROM carts c JOIN checkout_handoffs h ON h.cart_id=c.id AND h.tenant=c.tenant WHERE c.tenant=$1 AND c.id=$2 AND h.recovery_digest=$3 AND h.recovery_origin=$4 AND h.recovery_expires_at>clock_timestamp() FOR UPDATE OF c",
            k.tenant,
            k.cart,
            k.digest,
            k.origin
        )
        .fetch_optional(tx)
        .await?
    } else {
        sqlx::query_as!(
            CartRow,
            "SELECT c.id,c.tenant,c.token,c.data,c.revision,c.status FROM carts c JOIN checkout_handoffs h ON h.cart_id=c.id AND h.tenant=c.tenant WHERE c.tenant=$1 AND c.id=$2 AND h.recovery_digest=$3 AND h.recovery_origin=$4 AND h.recovery_expires_at>clock_timestamp()",
            k.tenant,
            k.cart,
            k.digest,
            k.origin
        )
        .fetch_optional(tx)
        .await?
    })
}

/// The recovery key is still valid (rechecked after waiting for the cart lock).
pub async fn recovery_valid(tx: &mut PgConnection, k: &RecoveryKey<'_>) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM checkout_handoffs WHERE tenant=$1 AND cart_id=$2 AND recovery_digest=$3 AND recovery_origin=$4 AND recovery_expires_at>clock_timestamp()) AS "valid!""#, k.tenant, k.cart, k.digest, k.origin)
        .fetch_one(tx)
        .await?)
}

/// Replaces the cart's ticket with a new ten-minute one.
pub async fn reissue(tx: &mut PgConnection, tenant: &str, cart: &str, digest: &str) -> Result<()> {
    sqlx::query!("UPDATE checkout_handoffs SET digest=$1,expires_at=now()+interval '10 minutes',consumed_at=NULL WHERE tenant=$2 AND cart_id=$3", digest, tenant, cart)
        .execute(tx)
        .await?;
    Ok(())
}

/// The cart of an open, unexpired ticket, row-locked before the ticket is consumed.
pub async fn lock_ticket_cart(
    tx: &mut PgConnection,
    tenant: &str,
    digest: &str,
) -> Result<Option<CartRow>> {
    Ok(sqlx::query_as!(
        CartRow,
        "SELECT c.id,c.tenant,c.token,c.data,c.revision,c.status FROM carts c JOIN checkout_handoffs h ON h.cart_id=c.id AND h.tenant=c.tenant WHERE h.tenant=$1 AND h.digest=$2 AND h.expires_at>now() AND h.consumed_at IS NULL FOR UPDATE OF c",
        tenant,
        digest
    )
    .fetch_optional(tx)
    .await?)
}

/// Consumes an open ticket; the count is 1 only for the request that consumed it.
pub async fn consume(tx: &mut PgConnection, tenant: &str, digest: &str) -> Result<u64> {
    Ok(sqlx::query!(
        "UPDATE checkout_handoffs SET consumed_at=clock_timestamp() WHERE tenant=$1 AND digest=$2 AND expires_at>clock_timestamp() AND consumed_at IS NULL",
        tenant,
        digest
    )
    .execute(tx)
    .await?
    .rows_affected())
}
