//! Personal second factor: encrypted TOTP secrets, one-use recovery digests, revisioned enrolment and one-use sign-in challenges. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// The live shop behind a sandbox; account security is managed only where there is none.
pub async fn live_shop(a: &State, tenant: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT live_tenant FROM shop_environments WHERE tenant=$1",
        tenant
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Whether the second factor is on, its revision and how many recovery codes remain.
pub struct Status {
    pub enabled: bool,
    pub revision: i64,
    pub remaining: i32,
}

/// The second-factor state of the user; `None` before any enrolment.
pub async fn status(a: &State, user: &str) -> Result<Option<Status>> {
    Ok(sqlx::query_as!(
        Status,
        r#"SELECT enabled,revision,jsonb_array_length(recovery_digests) AS "remaining!" FROM merchant_mfa WHERE user_id=$1"#,
        user
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Stores a pending (ten-minute) secret; returns the revision, or `None` when the factor is already on.
pub async fn begin_enrolment(
    tx: &mut sqlx::PgConnection,
    user: &str,
    cipher: &str,
) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "INSERT INTO merchant_mfa(user_id,pending_ciphertext,pending_expires_at) VALUES($1,$2,now()+interval '10 minutes') ON CONFLICT(user_id) DO UPDATE SET pending_ciphertext=EXCLUDED.pending_ciphertext,pending_expires_at=EXCLUDED.pending_expires_at WHERE NOT merchant_mfa.enabled RETURNING revision",
        user,
        cipher
    )
    .fetch_optional(tx)
    .await?)
}

/// A pending, unexpired enrolment, locked for confirmation.
pub struct Pending {
    pub pending_ciphertext: String,
    pub revision: i64,
}

/// Locks the user's pending enrolment.
pub async fn lock_pending(tx: &mut sqlx::PgConnection, user: &str) -> Result<Option<Pending>> {
    Ok(sqlx::query_as!(
        Pending,
        r#"SELECT pending_ciphertext AS "pending_ciphertext!",revision FROM merchant_mfa WHERE user_id=$1 AND NOT enabled AND pending_expires_at>now() FOR UPDATE"#,
        user
    )
    .fetch_optional(tx)
    .await?)
}

/// Turns the confirmed secret on with fresh recovery digests and the step it was confirmed with.
pub async fn enable(
    tx: &mut sqlx::PgConnection,
    user: &str,
    cipher: &str,
    digests: &Value,
    step: i64,
) -> Result<()> {
    sqlx::query!(
        "UPDATE merchant_mfa SET ciphertext=$2,enabled=true,pending_ciphertext=NULL,pending_expires_at=NULL,recovery_digests=$3,last_step=$4,revision=revision+1,updated_at=now() WHERE user_id=$1",
        user,
        cipher,
        digests,
        step
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// An enabled factor, locked while a code is consumed.
pub struct Enabled {
    pub ciphertext: String,
    pub last_step: i64,
    pub recovery_digests: Value,
}

/// Locks the user's enabled factor.
pub async fn lock_enabled(tx: &mut sqlx::PgConnection, user: &str) -> Result<Option<Enabled>> {
    Ok(sqlx::query_as!(
        Enabled,
        r#"SELECT ciphertext AS "ciphertext!",last_step,recovery_digests FROM merchant_mfa WHERE user_id=$1 AND enabled FOR UPDATE"#,
        user
    )
    .fetch_optional(tx)
    .await?)
}

/// Replaces the remaining recovery digests after one was used.
pub async fn set_recovery(tx: &mut sqlx::PgConnection, user: &str, digests: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE merchant_mfa SET recovery_digests=$2 WHERE user_id=$1",
        user,
        digests
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Records the time step of the last accepted code, so no code is accepted twice.
pub async fn set_last_step(tx: &mut sqlx::PgConnection, user: &str, step: i64) -> Result<()> {
    sqlx::query!(
        "UPDATE merchant_mfa SET last_step=$2 WHERE user_id=$1",
        user,
        step
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Turns the factor off when the revision still matches; returns the changed row count.
pub async fn disable(tx: &mut sqlx::PgConnection, user: &str, revision: i64) -> Result<u64> {
    Ok(sqlx::query!(
        "UPDATE merchant_mfa SET enabled=false,ciphertext=NULL,recovery_digests='[]',pending_ciphertext=NULL,pending_expires_at=NULL,revision=revision+1,updated_at=now() WHERE user_id=$1 AND revision=$2",
        user,
        revision
    )
    .execute(tx)
    .await?
    .rows_affected())
}

/// The person and shop a sign-in challenge belongs to.
pub struct Challenge {
    pub user_id: String,
    pub tenant: String,
}

/// Counts one attempt on an unexpired challenge with fewer than five attempts.
pub async fn attempt_challenge(a: &State, digest: &str) -> Result<Option<Challenge>> {
    Ok(sqlx::query_as!(
        Challenge,
        "UPDATE merchant_mfa_challenges SET attempts=attempts+1 WHERE digest=$1 AND expires_at>now() AND attempts<5 RETURNING user_id,tenant",
        digest
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Whether the challenge is still open and the user still holds an active grant in its shop.
pub async fn challenge_valid(
    tx: &mut sqlx::PgConnection,
    digest: &str,
    user: &str,
    tenant: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM merchant_mfa_challenges WHERE digest=$1 AND expires_at>now()) AND EXISTS(SELECT 1 FROM membership_grants WHERE user_id=$2 AND tenant=$3 AND active) AS "valid!""#,
        digest,
        user,
        tenant
    )
    .fetch_one(tx)
    .await?)
}

/// Removes a used challenge.
pub async fn delete_challenge(tx: &mut sqlx::PgConnection, digest: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM merchant_mfa_challenges WHERE digest=$1",
        digest
    )
    .execute(tx)
    .await?;
    Ok(())
}
