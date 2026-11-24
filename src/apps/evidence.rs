//! Private provenance-bearing app exports feed merchant retrieval and durable app events; never public PDP answers.
use super::gateway;
use crate::foundation::App;
use serde_json::json;
use veyndo_core::Result;
pub(crate) async fn collect_sources(a: &App) -> Result<()> {
    let configured = &crate::runtime_config::get().services;
    let ids = configured
        .as_object()
        .map(|v| v.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let rows = veyndo_app_platform::store::evidence::export_candidates(a, &ids).await?;
    let mut tasks = tokio::task::JoinSet::new();
    for (tenant, id, cursor) in rows {
        let app = a.clone();
        tasks.spawn(veyndo::tenant_scope::scoped(
            veyndo::tenant_scope::Scope::Tenant(tenant.clone()),
            async move { collect_one(&app, tenant, id, cursor).await },
        ));
        if tasks.len() >= 8 {
            let _ = tasks.join_next().await;
        }
    }
    while tasks.join_next().await.is_some() {}

    Ok(())
}
async fn collect_one(a: &App, t: String, id: String, cursor: i64) -> Result<()> {
    if veyndo_core::shop_scope::parent(a, &t).await?.is_some() {
        return Ok(());
    }
    veyndo_app_platform::store::evidence::touch_export(a, &t, &id).await?;
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
