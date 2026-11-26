//! Operator bootstrap and shop-creation steps of the control plane. The email lock is the server's own; accounts belong to identity and the operator grant and audit to shop.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;
pub(crate) use veyndo_identity::store::merchant_accounts::Credential as Account;
pub(crate) use veyndo_identity::store::provisioning::account_id_by_email;
pub(crate) use veyndo_shop::store::provisioning::{audit_operator_bootstrap, grant_operator};

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
pub(crate) async fn account_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Account>> {
    veyndo_identity::store::merchant_accounts::by_email(conn, email).await
}

pub(crate) async fn create_account(
    conn: &mut PgConnection,
    id: &str,
    email: &str,
    name: &str,
    password_hash: &str,
) -> Result<()> {
    veyndo_identity::store::provisioning::create_account(conn, id, email, name, password_hash).await
}

/// Locks a merchant account row for this transaction (fails when it does not exist).
pub(crate) async fn lock_account(conn: &mut PgConnection, id: &str) -> Result<()> {
    veyndo_identity::store::merchant_accounts::lock(conn, id).await
}

/// Records an operator-created shop.
pub(crate) async fn audit_shop_created(
    conn: &mut PgConnection,
    actor: &str,
    tenant: &str,
    data: Value,
) -> Result<()> {
    veyndo_shop::store::provisioning::audit_shop_created(conn, actor, tenant, data).await
}
