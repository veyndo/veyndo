//! Model budgets of agent keys: the approved budget of an unexpired key, the locked allocation counters and their increase.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;

/// An agent key's budget and what it has allocated so far.
pub struct KeyUsage {
    pub agent_budget: Option<Value>,
    pub agent_tokens_allocated: i64,
    pub agent_cost_allocated: i64,
}

/// The approved budget of an unexpired agent key of a live shop. A key without a budget fails to decode.
pub async fn approved<'e>(
    db: impl Executor<'e, Database = Postgres>,
    live: &str,
    key: Option<&str>,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT agent_budget AS "agent_budget!" FROM integration_keys WHERE tenant=$1 AND id=$2 AND expires_at>now()"#,
        live,
        key
    )
    .fetch_optional(db)
    .await?)
}

/// Locks an unexpired user agent key whose creator is still an active member.
pub async fn lock(conn: &mut PgConnection, live: &str, id: &str) -> Result<Option<KeyUsage>> {
    Ok(sqlx::query_as!(
        KeyUsage,
        "SELECT i.agent_budget,i.agent_tokens_allocated,i.agent_cost_allocated FROM integration_keys i JOIN membership_grants m ON m.tenant=i.tenant AND m.user_id=i.user_id AND m.active WHERE i.tenant=$1 AND i.id=$2 AND i.expires_at>now() AND i.app_id IS NULL FOR UPDATE OF i",
        live,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Adds a reservation to a key's allocated tokens and cost.
pub async fn allocate(
    conn: &mut PgConnection,
    live: &str,
    id: &str,
    tokens: i64,
    cost: i64,
) -> Result<()> {
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
