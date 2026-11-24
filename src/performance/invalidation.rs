//! Commit-only outbox notifications eagerly evict replica caches; authoritative version probes survive missed events.
use crate::runtime_config;
use serde_json::Value;
use sqlx::postgres::PgListener;
pub(crate) fn start() {
    veyndo::tenant_scope::spawn(async move {
        let url = &runtime_config::get().listener_url;
        loop {
            super::reads().clear();
            let connected = PgListener::connect(url).await;
            if let Ok(mut listener) = connected
                && listener.listen("veyndo_read_invalidation").await.is_ok()
            {
                while let Ok(Some(event)) = listener.try_recv().await {
                    if let Ok(v) = serde_json::from_str::<Value>(event.payload())
                        && let Some(t) = v["tenant"].as_str()
                    {
                        super::reads().invalidate(t);
                    } else {
                        super::reads().clear();
                    }
                }
            }
            // Restart clears decoded entries. No stale time window: every normal hit still probes SQL versions.
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}
