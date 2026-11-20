//! Private provenance-bearing app sources for merchant retrieval; never public PDP answers.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Result, State};

/// The best matching private sources of active apps for a merchant query.
pub async fn private_evidence(a: &State, t: &str, query: &str) -> Result<Value> {
    let rows=sqlx::query("SELECT e.* FROM app_evidence e JOIN app_packages p ON p.tenant=e.tenant AND p.id=e.app AND p.active WHERE e.tenant=$1 ORDER BY ts_rank_cd(to_tsvector('simple',e.title||' '||e.body),plainto_tsquery('simple',$2)) DESC,e.updated_at DESC LIMIT 12").bind(t).bind(query).fetch_all(&a.db).await?;
    Ok(json!(rows.iter().map(|r|json!({"app":r.get::<String,_>("app"),"sourceId":r.get::<String,_>("source_id"),"title":r.get::<String,_>("title"),"text":r.get::<String,_>("body").chars().take(2500).collect::<String>(),"sourceUrl":r.get::<String,_>("source_url"),"metadata":r.get::<Value,_>("metadata"),"digest":r.get::<String,_>("digest"),"visibility":"merchant-private","trustedInstructions":false})).collect::<Vec<_>>()))
}
/// The private sources of active apps and the products they relate to.
pub async fn private_graph(a: &State, t: &str) -> Result<Value> {
    let active: Vec<String> =
        sqlx::query_scalar("SELECT id FROM app_packages WHERE tenant=$1 AND active")
            .bind(t)
            .fetch_all(&a.db)
            .await?;
    let sources:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('app',app,'sourceId',source_id,'title',title,'kind',kind) FROM app_evidence WHERE tenant=$1 AND app=ANY($2) ORDER BY app,source_id LIMIT 100").bind(t).bind(&active).fetch_all(&a.db).await?;
    let products:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('app',r.data->>'app','sourceId',r.data->>'sourceId','productId',p.id) FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=r.target_id JOIN app_evidence e ON e.tenant=r.tenant AND e.app=r.data->>'app' AND e.source_id=r.data->>'sourceId' WHERE r.tenant=$1 AND r.kind='REFERENCES_PRODUCT' AND e.app=ANY($2) ORDER BY e.app,e.source_id,p.id LIMIT 100").bind(t).bind(&active).fetch_all(&a.db).await?;
    Ok(json!({"sources":sources,"productRelations":products,"visibility":"merchant-private"}))
}
