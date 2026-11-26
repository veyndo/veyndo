//! Consent-bound private cart preferences: revision-checked write, erasure and the request-only advice context.
use crate::domain::preference_graph::Graph;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_core::document::view;
use veyndo_core::{Result, State, bad, conflict, hash};

/// The saved graph of a cart with its revision, or an empty one.
pub async fn read(a: &State, tenant: &str, cart: &str) -> Result<Value> {
    let r = sqlx::query!(
        "SELECT data,revision FROM private_preferences WHERE tenant=$1 AND cart_id=$2",
        tenant,
        cart
    )
    .fetch_optional(&a.db)
    .await?;
    Ok(r.map(|r|{let d=view::<Saved>(&r.data);json!({"graph":d.graph,"useForAdvice":d.use_for_advice,"revision":r.revision,"scope":"private cart context"})}).unwrap_or(json!({"graph":{"nodes":[],"edges":[]},"revision":0,"scope":"private cart context"})))
}

/// The saved preference document.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Saved {
    graph: Value,
    use_for_advice: Value,
}

/// Replaces the graph at the expected revision; referenced products are held against deletion.
pub async fn write(
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    cart: &str,
    graph: &Graph,
    use_for_advice: bool,
    revision: i64,
) -> Result<()> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,18))",
        format!("{}:{}", tenant, hash(cart))
    )
    .execute(&mut **tx)
    .await?;
    for n in &graph.nodes {
        if let Some(id) = &n.product_id {
            sqlx::query_scalar!(
                "SELECT id FROM products WHERE tenant=$1 AND id=$2 FOR KEY SHARE",
                tenant,
                id
            )
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(bad("Preference product disappeared"))?;
        }
    }
    let current = sqlx::query_scalar!(
        "SELECT revision FROM private_preferences WHERE tenant=$1 AND cart_id=$2 FOR UPDATE",
        tenant,
        cart
    )
    .fetch_optional(&mut **tx)
    .await?;
    if current.unwrap_or(0) != revision {
        return Err(conflict("Preference revision changed"));
    }
    sqlx::query!("INSERT INTO private_preferences(tenant,cart_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,cart_id) DO UPDATE SET data=EXCLUDED.data,revision=private_preferences.revision+1,updated_at=now()",
        tenant, cart, json!({"graph":graph,"useForAdvice":use_for_advice})).execute(&mut **tx).await?;
    Ok(())
}

/// Erase explicitly resolved cart capabilities together; never group by caller-supplied session.
pub async fn forget_many(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
    carts: &[String],
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM private_preferences WHERE tenant=$1 AND cart_id=ANY($2)",
        tenant,
        carts
    )
    .execute(&mut **tx)
    .await?;
    // Erasure invalidates inference rather than silently introducing differential attrition into a randomized test.
    sqlx::query!("WITH removed AS (DELETE FROM intelligence_assignments WHERE tenant=$1 AND cart_id=ANY($2) RETURNING experiment_id) UPDATE intelligence_experiments e SET withdrawn=e.withdrawn+r.count FROM (SELECT experiment_id,count(*)::int AS count FROM removed GROUP BY experiment_id) r WHERE e.tenant=$1 AND e.id=r.experiment_id",
        tenant, carts).execute(&mut **tx).await?;
    sqlx::query!("UPDATE knowledge_relations r SET data=jsonb_set(jsonb_set(data,'{causalUpliftProven}','false'),'{interval95}','null') WHERE r.tenant=$1 AND r.kind='EXPERIMENT_RESULT' AND EXISTS(SELECT 1 FROM intelligence_experiments e WHERE e.tenant=r.tenant AND e.id=r.source_id AND e.withdrawn>0)",
        tenant).execute(&mut **tx).await?;
    Ok(())
}

/// A request-only source stamp; never persisted as a second memory ledger or exposed to the model.
pub struct AdviceContext {
    pub graph: Value,
    stamp: Option<String>,
}
impl AdviceContext {
    /// No private context: nothing was consented, saved or still fresh.
    pub fn empty() -> Self {
        AdviceContext {
            graph: Value::Null,
            stamp: None,
        }
    }
    /// Whether both captures saw the same saved graph.
    pub fn unchanged(&self, other: &AdviceContext) -> bool {
        self.stamp == other.stamp
    }
}

/// The graph a shopper released for advice within the last thirty days.
pub async fn advice(a: &State, tenant: &str, cart: &str) -> Result<AdviceContext> {
    let row = sqlx::query!(r#"SELECT data->'graph' AS "graph!",revision,updated_at::text AS "changed!" FROM private_preferences WHERE tenant=$1 AND cart_id=$2 AND data->'useForAdvice'='true'::jsonb AND updated_at>now()-interval '30 days'"#,
        tenant, cart).fetch_optional(&a.db).await?;
    Ok(row
        .map(|row| {
            let graph = row.graph;
            let stamp = hash(&json!([tenant, cart, row.revision, row.changed, graph]).to_string());
            AdviceContext {
                graph,
                stamp: Some(stamp),
            }
        })
        .unwrap_or_else(AdviceContext::empty))
}
