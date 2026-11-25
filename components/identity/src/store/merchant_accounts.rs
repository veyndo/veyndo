//! Merchant accounts: creation for self-registration, invitations and the trusted identity broker, password lookups, the account lock and the daily allowance of broker inference.
use sqlx::{Executor, Postgres};
use veyndo_core::{Result, State, uid};

/// Creates a personal merchant account; false when the email is already registered.
pub async fn register(
    tx: &mut sqlx::PgConnection,
    user: &str,
    (email, name): (&str, &str),
    password_hash: String,
) -> Result<bool> {
    let changed = sqlx::query("INSERT INTO merchant_users(id,email,name,password_hash) VALUES($1,$2,$3,$4) ON CONFLICT(email) DO NOTHING")
        .bind(user)
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .execute(tx)
        .await?;
    Ok(changed.rows_affected() == 1)
}

/// The account of a broker-verified email under the email lock, created with an unusable password when new; the broker subject is linked once.
/// Returns the user and the user a previous link of the same subject named.
pub async fn broker_account(
    tx: &mut sqlx::PgConnection,
    (issuer, subject): (&str, &str),
    (email, name): (&str, &str),
    random_password: String,
) -> Result<(String, Option<String>)> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,42))")
        .bind(email)
        .execute(&mut *tx)
        .await?;
    // Only the configured broker can assert verified ownership of this email; arbitrary clients cannot link accounts.
    let user: String=sqlx::query_scalar("INSERT INTO merchant_users(id,email,name,password_hash,password_initialized,email_verified_at) VALUES($1,$2,$3,$4,false,now()) ON CONFLICT(email) DO UPDATE SET email=EXCLUDED.email,email_verified_at=COALESCE(merchant_users.email_verified_at,now()) RETURNING id")
        .bind(uid()).bind(email).bind(name).bind(random_password).fetch_one(&mut *tx).await?;
    let linked: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM merchant_identities WHERE issuer=$1 AND subject=$2",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_optional(&mut *tx)
    .await?;
    Ok((user, linked))
}

/// Links the broker subject to the user; an existing link is kept.
pub async fn link(
    tx: &mut sqlx::PgConnection,
    (issuer, subject): (&str, &str),
    user: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO merchant_identities(issuer,subject,user_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(issuer).bind(subject).bind(user).execute(tx).await?;
    Ok(())
}

/// The first shop the user holds an active grant in.
pub async fn first_shop<'e>(
    db: impl Executor<'e, Database = Postgres>,
    user: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT tenant FROM membership_grants WHERE user_id=$1 AND active ORDER BY tenant LIMIT 1",
    )
    .bind(user)
    .fetch_optional(db)
    .await?)
}

/// Whether the shop exists, and whether the user is its active owner.
pub async fn ownership(
    tx: &mut sqlx::PgConnection,
    shop: &str,
    user: &str,
) -> Result<(bool, bool)> {
    let exists: bool = veyndo_core::store::tenants::exists(&mut *tx, shop).await?;
    let owned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND role='owner' AND active)").bind(shop).bind(user).fetch_one(&mut *tx).await?;
    Ok((exists, owned))
}

/// Counts one broker inference call of the email today and returns the day's count.
pub async fn count_inference(a: &State, email: &str) -> Result<i32> {
    Ok(sqlx::query_scalar("INSERT INTO identity_inference_daily(email,day,calls) VALUES($1,current_date,1) ON CONFLICT(email,day) DO UPDATE SET calls=identity_inference_daily.calls+1 RETURNING calls").bind(email).fetch_one(&a.db).await?)
}

/// The id and password hash of the account behind an email.
pub struct Credential {
    pub id: String,
    pub password_hash: String,
}

/// The account behind an email, for a password check.
pub async fn by_email<'e>(
    db: impl Executor<'e, Database = Postgres>,
    email: &str,
) -> Result<Option<Credential>> {
    Ok(sqlx::query_as!(
        Credential,
        "SELECT id,password_hash FROM merchant_users WHERE email=$1",
        email
    )
    .fetch_optional(db)
    .await?)
}

/// Creates an account with a password; a duplicate email fails the statement.
pub async fn create(
    tx: &mut sqlx::PgConnection,
    user: &str,
    (email, name): (&str, &str),
    password_hash: String,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO merchant_users(id,email,name,password_hash) VALUES($1,$2,$3,$4)",
        user,
        email,
        name,
        password_hash
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Locks the account row until the transaction ends; serialises session issue and credential changes of one person.
pub async fn lock(tx: &mut sqlx::PgConnection, user: &str) -> Result<()> {
    sqlx::query_scalar!("SELECT id FROM merchant_users WHERE id=$1 FOR UPDATE", user)
        .fetch_one(tx)
        .await?;
    Ok(())
}

/// The password hash of the account, locked for a credential change.
pub async fn password_hash_for_update(tx: &mut sqlx::PgConnection, user: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        "SELECT password_hash FROM merchant_users WHERE id=$1 FOR UPDATE",
        user
    )
    .fetch_one(tx)
    .await?)
}

/// The email of the account.
pub async fn email(tx: &mut sqlx::PgConnection, user: &str) -> Result<String> {
    Ok(
        sqlx::query_scalar!("SELECT email FROM merchant_users WHERE id=$1", user)
            .fetch_one(tx)
            .await?,
    )
}
