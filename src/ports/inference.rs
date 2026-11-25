//! The inference component's host: agent key budgets from identity and the operator configuration, read for model admission.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_cognition::ports::{InferenceHost, KeyUsage, PlatformAi};
use veyndo_cognition::scoped_pool::ScopedPool;
use veyndo_core::PortFuture;
use veyndo_identity::store::agent_budgets;

/// Agent keys (identity) and the platform AI row (shop) for model admission.
pub(crate) struct Host;

impl InferenceHost for Host {
    fn approved_budget<'a>(
        &'a self,
        db: &'a ScopedPool,
        live: &'a str,
        key: Option<&'a str>,
    ) -> PortFuture<'a, Option<Value>> {
        Box::pin(agent_budgets::approved(db, live, key))
    }

    fn lock_key<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        live: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Option<KeyUsage>> {
        Box::pin(async move {
            Ok(agent_budgets::lock(conn, live, id)
                .await?
                .map(|row| KeyUsage {
                    agent_budget: row.agent_budget,
                    agent_tokens_allocated: row.agent_tokens_allocated,
                    agent_cost_allocated: row.agent_cost_allocated,
                }))
        })
    }

    fn allocate<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        live: &'a str,
        id: &'a str,
        tokens: i64,
        cost: i64,
    ) -> PortFuture<'a, ()> {
        Box::pin(agent_budgets::allocate(conn, live, id, tokens, cost))
    }

    fn platform_ai<'a>(&'a self, db: &'a ScopedPool) -> PortFuture<'a, PlatformAi> {
        Box::pin(async move {
            let row = veyndo_shop::store::platform_ai::read(db).await?;
            Ok(PlatformAi {
                data: row.data,
                secrets: row.secrets,
            })
        })
    }
}
