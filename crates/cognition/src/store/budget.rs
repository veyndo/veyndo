//! Agent key budgets: the approved model of a key, the live shop's stop switch and the reservation of tokens and cost with its audit event.
use crate::scoped_pool::ScopedPool;
use serde_json::Value;
use sqlx::PgConnection;

/// The approved budget of an unexpired agent key, resolved through a preview environment to its live shop. A key without a budget fails to decode.
pub async fn approved(
    db: &ScopedPool,
    tenant: &str,
    key: Option<&str>,
) -> sqlx::Result<Option<Value>> {
    sqlx::query_scalar!(
        r#"SELECT agent_budget AS "agent_budget!" FROM integration_keys WHERE tenant=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) AND id=$2 AND expires_at>now()"#,
        tenant,
        key
    )
    .fetch_optional(db)
    .await
}

/// The live shop of a tenant or its environment, with its agent stop switch.
pub struct LiveShop {
    pub id: String,
    pub agents_stopped: bool,
}

/// Locks the live shop row against concurrent stop changes.
pub async fn lock_shop(conn: &mut PgConnection, tenant: &str) -> sqlx::Result<LiveShop> {
    sqlx::query_as!(
        LiveShop,
        "SELECT id,agents_stopped FROM tenants WHERE id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) FOR NO KEY UPDATE",
        tenant
    )
    .fetch_one(conn)
    .await
}

/// An agent key's budget and what it has allocated so far.
pub struct KeyUsage {
    pub agent_budget: Option<Value>,
    pub agent_tokens_allocated: i64,
    pub agent_cost_allocated: i64,
}

/// Locks an unexpired agent key whose creator is still an active member.
pub async fn lock_key(
    conn: &mut PgConnection,
    live: &str,
    id: &str,
) -> sqlx::Result<Option<KeyUsage>> {
    sqlx::query_as!(
        KeyUsage,
        "SELECT i.agent_budget,i.agent_tokens_allocated,i.agent_cost_allocated FROM integration_keys i JOIN membership_grants m ON m.tenant=i.tenant AND m.user_id=i.user_id AND m.active WHERE i.tenant=$1 AND i.id=$2 AND i.expires_at>now() AND i.app_id IS NULL FOR UPDATE OF i",
        live,
        id
    )
    .fetch_optional(conn)
    .await
}

/// Adds a reservation to a key's allocated tokens and cost.
pub async fn allocate(
    conn: &mut PgConnection,
    live: &str,
    id: &str,
    tokens: i64,
    cost: i64,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE integration_keys SET agent_tokens_allocated=agent_tokens_allocated+$3,agent_cost_allocated=agent_cost_allocated+$4 WHERE tenant=$1 AND id=$2",
        live,
        id,
        tokens,
        cost
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Records the reservation as an `intelligence.agent.allocated` outbox event in the same transaction.
pub async fn audit(conn: &mut PgConnection, live: &str, data: &Value) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO outbox(tenant,kind,data) VALUES($1,'intelligence.agent.allocated',$2)",
        live,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}
