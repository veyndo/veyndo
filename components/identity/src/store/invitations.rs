//! Single-use, expiring workspace invitations: creation, the pending list, locked redemption and revocation. Columns checked at compile time.
use veyndo_core::{Result, State};

/// Stores a new invitation under the digest of its token.
pub async fn create(
    tx: &mut sqlx::PgConnection,
    (id, digest): (&str, &str),
    (tenant, email, role): (&str, &str, &str),
    created_by: &str,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO user_invites(id,digest,tenant,email,role,created_by) VALUES($1,$2,$3,$4,$5,$6)",
        id,
        digest,
        tenant,
        email,
        role,
        created_by
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Marks an unredeemed invitation of the shop as used, so its token no longer works.
pub async fn revoke(a: &State, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE user_invites SET redeemed_at=now() WHERE tenant=$1 AND id=$2 AND redeemed_at IS NULL",
        tenant,
        id
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// An open invitation as the member list shows it.
pub struct PendingInvite {
    pub id: String,
    pub email: String,
    pub role: String,
    pub expires: String,
}

/// The shop's open invitations, soonest expiry first.
pub async fn pending(a: &State, tenant: &str) -> Result<Vec<PendingInvite>> {
    Ok(sqlx::query_as!(
        PendingInvite,
        r#"SELECT id,email,role,expires_at::text AS "expires!" FROM user_invites WHERE tenant=$1 AND redeemed_at IS NULL AND expires_at>now() ORDER BY expires_at"#,
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// The shop of an open invitation, read before the shop lock is taken.
pub async fn pending_tenant(tx: &mut sqlx::PgConnection, digest: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT tenant FROM user_invites WHERE digest=$1 AND redeemed_at IS NULL AND expires_at>now()",
        digest
    )
    .fetch_optional(tx)
    .await?)
}

/// An open invitation locked for redemption.
pub struct Invite {
    pub id: String,
    pub tenant: String,
    pub email: String,
    pub role: String,
    pub created_by: String,
}

/// Locks an open invitation for redemption.
pub async fn lock_pending(tx: &mut sqlx::PgConnection, digest: &str) -> Result<Option<Invite>> {
    Ok(sqlx::query_as!(
        Invite,
        "SELECT id,tenant,email,role,created_by FROM user_invites WHERE digest=$1 AND redeemed_at IS NULL AND expires_at>now() FOR UPDATE",
        digest
    )
    .fetch_optional(tx)
    .await?)
}

/// Marks a redeemed invitation as used.
pub async fn redeem(tx: &mut sqlx::PgConnection, id: &str) -> Result<()> {
    sqlx::query!("UPDATE user_invites SET redeemed_at=now() WHERE id=$1", id)
        .execute(tx)
        .await?;
    Ok(())
}
