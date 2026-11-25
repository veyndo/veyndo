//! Operator and merchant-account writes of the control plane: operator bootstrap, shop creation audit and account lookups. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Serializes the operator bootstrap of one email address.
pub(crate) async fn lock_operator_email(conn: &mut PgConnection, email: &str) -> Result<()> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,29))",
        email
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// A personal merchant account found by email.
pub(crate) struct Account {
    pub id: String,
    pub password_hash: String,
}

pub(crate) async fn account_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Account>> {
    Ok(sqlx::query_as!(
        Account,
        "SELECT id,password_hash FROM merchant_users WHERE email=$1",
        email
    )
    .fetch_optional(conn)
    .await?)
}

pub(crate) async fn account_id_by_email<'e>(
    db: impl sqlx::PgExecutor<'e>,
    email: &str,
) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT id FROM merchant_users WHERE email=$1", email)
            .fetch_optional(db)
            .await?,
    )
}

pub(crate) async fn create_account(
    conn: &mut PgConnection,
    id: &str,
    email: &str,
    name: &str,
    password_hash: &str,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO merchant_users(id,email,name,password_hash) VALUES($1,$2,$3,$4)",
        id,
        email,
        name,
        password_hash
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Locks a merchant account row for this transaction (fails when it does not exist).
pub(crate) async fn lock_account(conn: &mut PgConnection, id: &str) -> Result<()> {
    sqlx::query_scalar!("SELECT id FROM merchant_users WHERE id=$1 FOR UPDATE", id)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// Grants (or re-activates) the operator role.
pub(crate) async fn grant_operator(conn: &mut PgConnection, user: &str) -> Result<()> {
    sqlx::query!("INSERT INTO platform_operators(user_id) VALUES($1) ON CONFLICT(user_id) DO UPDATE SET active=true", user)
        .execute(conn)
        .await?;
    Ok(())
}

/// Records the operator bootstrap.
pub(crate) async fn audit_operator_bootstrap(conn: &mut PgConnection, user: &str) -> Result<()> {
    sqlx::query!("INSERT INTO platform_audit(actor,action,data) VALUES($1,'operator.bootstrap',jsonb_build_object('userId',$1::text))", user)
        .execute(conn)
        .await?;
    Ok(())
}

/// Records an operator-created shop.
pub(crate) async fn audit_shop_created(
    conn: &mut PgConnection,
    actor: &str,
    tenant: &str,
    data: Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO platform_audit(actor,action,tenant,data) VALUES($1,'shop.created',$2,$3)",
        actor,
        tenant,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}
