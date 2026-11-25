//! Operator control-plane records: per-shop AI limits, the shop lifecycle status, the audit trail, the operator's account and the shop dossier. Accounts, products and AI usage come from the identity, catalog and core stores, customers, apps and receipts through the dependants port.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

pub use veyndo_core::store::resource_limits::Limit;

/// The stored AI limit of a shop; none while it uses the defaults.
pub async fn limit(a: &State, tenant: &str) -> Result<Option<Limit>> {
    veyndo_core::store::resource_limits::limit(a, tenant).await
}

/// The interactive AI attempts of a shop on the current UTC day.
pub async fn used_today(a: &State, tenant: &str) -> Result<Option<i64>> {
    veyndo_core::store::ai_usage::used_today(&a.db, tenant).await
}

/// The revision of a shop's stored AI limit, on the caller's connection.
pub async fn limit_revision(conn: &mut PgConnection, tenant: &str) -> Result<Option<i64>> {
    veyndo_core::store::resource_limits::revision(conn, tenant).await
}

/// Inserts the limit at `revision`, or replaces it and advances the stored revision.
pub async fn upsert_limit(
    conn: &mut PgConnection,
    tenant: &str,
    daily_ai: i64,
    revision: i64,
) -> Result<()> {
    veyndo_core::store::resource_limits::upsert(conn, tenant, daily_ai, revision).await
}

/// Records an operator action in the control-plane audit trail.
pub async fn audit(
    conn: &mut PgConnection,
    actor: &str,
    action: &str,
    tenant: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO platform_audit(actor,action,tenant,data) VALUES($1,$2,$3,$4)",
        actor,
        action,
        tenant,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// A live shop's lifecycle status and its revision.
pub struct Status {
    pub status: String,
    pub status_revision: i64,
}

/// The status of a live shop (never a sandbox) under an update lock.
pub async fn lock_status(conn: &mut PgConnection, id: &str) -> Result<Option<Status>> {
    Ok(sqlx::query_as!(
        Status,
        "SELECT status,status_revision FROM tenants t WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id) FOR UPDATE",
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Sets the shop status and advances its revision.
pub async fn set_status(conn: &mut PgConnection, id: &str, status: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE tenants SET status=$2,status_revision=status_revision+1,status_updated_at=now() WHERE id=$1",
        id,
        status
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The operator's merchant account.
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
}

/// The merchant account of a user.
pub async fn account(a: &State, user: &str) -> Result<Account> {
    let account = veyndo_identity::store::shop_reads::account(a, user).await?;
    Ok(Account {
        id: account.id,
        name: account.name,
        email: account.email,
    })
}

/// One control-plane action.
pub struct AuditEntry {
    pub id: i64,
    pub actor: Option<String>,
    pub action: String,
    pub tenant: Option<String>,
    pub time: String,
}

/// The latest hundred control-plane actions, newest first.
pub async fn audit_trail(a: &State) -> Result<Vec<AuditEntry>> {
    Ok(sqlx::query_as!(
        AuditEntry,
        r#"SELECT id,actor,action,tenant,created_at::text AS "time!" FROM platform_audit ORDER BY id DESC LIMIT 100"#
    )
    .fetch_all(&a.db)
    .await?)
}

/// When a shop was registered and its lifecycle status.
pub struct Registration {
    pub registered: String,
    pub status: String,
    pub status_revision: i64,
}

/// The registration of a shop.
pub async fn registration(a: &State, id: &str) -> Result<Registration> {
    Ok(sqlx::query_as!(
        Registration,
        r#"SELECT created_at::text AS "registered!",status,status_revision FROM tenants WHERE id=$1"#,
        id
    )
    .fetch_one(&a.db)
    .await?)
}

/// The products, customers and active apps of a shop.
pub struct Counts {
    pub products: i64,
    pub customers: i64,
    pub apps: i64,
}

/// How many products, customers and active apps a shop has.
pub async fn counts(a: &State, id: &str) -> Result<Counts> {
    let ids = [id.to_owned()];
    let products = veyndo_catalog::store::shop_reads::count_by_shop(a, &ids)
        .await?
        .first()
        .map_or(0, |(_, count)| *count);
    let held = crate::ports::dependants()?
        .activity(&ids, 1)
        .await?
        .remove(id)
        .unwrap_or_default();
    Ok(Counts {
        products,
        customers: held.customers,
        apps: held.apps,
    })
}

/// A member of a shop's access roster.
pub struct Member {
    pub name: String,
    pub email: String,
    pub role: String,
    pub active: bool,
}

/// Up to a hundred members of a shop by name.
pub async fn members(a: &State, id: &str) -> Result<Vec<Member>> {
    Ok(veyndo_identity::store::shop_reads::roster(a, id)
        .await?
        .into_iter()
        .map(|m| Member {
            name: m.name,
            email: m.email,
            role: m.role,
            active: m.active,
        })
        .collect())
}

/// A sales channel with its stored configuration.
pub struct Channel {
    pub id: String,
    pub data: Value,
}

/// Up to a hundred sales channels of a shop by ID.
pub async fn channels(a: &State, id: &str) -> Result<Vec<Channel>> {
    Ok(sqlx::query_as!(
        Channel,
        "SELECT id,data FROM sales_channels WHERE tenant=$1 ORDER BY id LIMIT 100",
        id
    )
    .fetch_all(&a.db)
    .await?)
}

/// The validated business identity of a shop's receipts.
pub async fn receipt_settings(id: &str) -> Result<Option<Value>> {
    crate::ports::dependants()?.receipt_identity(id).await
}
