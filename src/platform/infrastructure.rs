//! Live control-plane telemetry: database probes and pool/cache diagnostics, explicit process-only scope.
use super::auth;
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::env;
use veyndo_core::Result;
use veyndo_core::document::merge;
use veyndo_core::request_context::RequestContext;
/// The outcome of the database probe, added to the pool diagnostics.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Probe {
    healthy: bool,
    probe_ms: u128,
}
pub(super) async fn snapshot(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    auth::actor(&h)?;
    let start = std::time::Instant::now();
    let (mut database, pending) = veyndo_shop::store::operators::database_probe(&a).await?;
    merge(
        &mut database,
        Probe {
            healthy: true,
            probe_ms: start.elapsed().as_millis(),
        },
    )?;
    let traffic = veyndo_core::store::channel_metrics::traffic(&a.db, None, false).await?;
    let qdrant_start = std::time::Instant::now();
    let mut qdrant_request = a
        .http
        .get(format!(
            "{}/healthz",
            env::var("QDRANT_URL")
                .unwrap_or("http://127.0.0.1:6333".into())
                .trim_end_matches('/')
        ))
        .timeout(std::time::Duration::from_secs(2));
    if let Ok(key) = env::var("QDRANT_API_KEY") {
        qdrant_request = qdrant_request.header("api-key", key);
    }
    let qdrant = qdrant_request.send().await;
    Ok(Json(
        json!({"generatedAt":chrono::Utc::now().to_rfc3339(),"database":database,
        "resources":super::resources::snapshot().await,"process":{"poolConnections":a.db.size(),"poolIdle":a.db.num_idle(),"readCache":crate::performance::reads().snapshot(),"admission":a.admission.snapshot(),"rowSecurity":{"strictRuntime":env::var("DB_RLS_REQUIRED").as_deref()==Ok("true"),"runtimeCredentialSeparated":env::var("DATABASE_RUNTIME_URL").is_ok()}},
        "qdrant":{"healthy":qdrant.is_ok_and(|r|r.status().is_success()),"probeMs":qdrant_start.elapsed().as_millis()},
        "pendingEvents":pending,"channels":traffic,
        "scope":"pool/cache are this process; HTTP counters are persisted fleet diagnostics, not visitors; CPU/RAM are optional Linux container readings; percentiles unavailable"}),
    ))
}

#[cfg(test)]
mod tests {
    use super::Probe;
    use serde_json::json;
    use veyndo_core::document::merge;

    #[test]
    fn probe_keeps_the_indexed_shape() {
        let database = json!({"poolSize":4,"idle":2});
        let mut old = database.clone();
        old["healthy"] = json!(true);
        old["probeMs"] = json!(17_u128);
        let mut new = database;
        merge(
            &mut new,
            Probe {
                healthy: true,
                probe_ms: 17,
            },
        )
        .unwrap();
        assert_eq!(new, old);
    }
}
