//! Private provenance-bearing app exports feed merchant retrieval and durable app events; never public PDP answers.
use super::*;
pub(crate) async fn collect_sources(a: &App) -> Result<()> {
    let configured = &crate::runtime_config::get().services;
    let ids = configured
        .as_object()
        .map(|v| v.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let rows=sqlx::query("SELECT p.tenant,p.id,coalesce(c.cursor,0) AS cursor FROM app_packages p LEFT JOIN app_export_cursors c ON c.tenant=p.tenant AND c.app=p.id WHERE p.active AND p.id=ANY($1) AND p.manifest->'permissions' ? 'knowledge.write' ORDER BY coalesce(c.last_poll,'epoch'),p.tenant,p.id LIMIT 100").bind(ids).fetch_all(&a.db).await?;
    let mut tasks = tokio::task::JoinSet::new();
    for row in rows {
        let app = a.clone();
        let tenant: String = row.get("tenant");
        tasks.spawn(veyndo::tenant_scope::scoped(
            veyndo::tenant_scope::Scope::Tenant(tenant),
            async move { collect_one(&app, row).await },
        ));
        if tasks.len() >= 8 {
            let _ = tasks.join_next().await;
        }
    }
    while tasks.join_next().await.is_some() {}

    Ok(())
}
async fn collect_one(a: &App, r: sqlx::postgres::PgRow) -> Result<()> {
    let t: String = r.get("tenant");
    let id: String = r.get("id");
    let cursor: i64 = r.get("cursor");
    if staging::parent(a, &t).await?.is_some() {
        return Ok(());
    }
    sqlx::query("INSERT INTO app_export_cursors(tenant,app,last_poll) VALUES($1,$2,now()) ON CONFLICT(tenant,app) DO UPDATE SET last_poll=now()").bind(&t).bind(&id).execute(&a.db).await?;
    let result = gateway::service_call(a, &t, &id, "exports", &json!({"cursor":cursor})).await;
    if let Ok(v) = result
        && ingest(a, &t, &id, cursor, &v).await.is_err()
    {
        eprintln!("App export rejected: {id}");
    }
    Ok(())
}

use veyndo_app_platform::store::evidence::ingest;
pub(crate) use veyndo_app_platform::store::evidence::{
    private_evidence, private_graph, purge_sources,
};
