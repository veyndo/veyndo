//! Bounded current-right work items come from native rows; no inferred revenue, customer data or parallel task store.
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{SCOPES, allowed};

/// A sample of what waits for the person, limited to the areas their rights cover.
pub async fn load(a: &State, h: &RequestContext, t: &str, actor: &str) -> Result<Value> {
    let mut items = vec![];
    if allowed(h, "catalog.read") {
        let ids = veyndo_catalog::store::workspace::out_of_stock(a, t).await?;
        for id in ids.into_iter().take(3) {
            items.push(
                json!({"id":format!("stock:{id}"),"kind":"stock","tab":"productData","entity":id}),
            );
        }
    }
    if allowed(h, "assistant.use") && allowed(h, "knowledge.read") {
        let scopes = json!(SCOPES.iter().filter(|s| allowed(h, s)).collect::<Vec<_>>());
        let ids =
            veyndo_intelligence::store::workspace::unsettled_runs(a, t, actor, &scopes).await?;
        for id in ids {
            items.push(json!({"id":format!("run:{id}"),"kind":"run","tab":"assistant","runId":id}));
        }
    }
    if allowed(h, "catalog.write") {
        let ids = veyndo_catalog::store::workspace::reviewable_translations(a, t).await?;
        for id in ids {
            items.push(json!({"id":format!("translation:{id}"),"kind":"translation","tab":"commerce","translationJob":id}));
        }
    }
    if allowed(h, "apps.manage") {
        let ids = veyndo_app_platform::store::workspace::failed_job_apps(a, t).await?;
        for id in ids {
            items.push(json!({"id":format!("app:{id}"),"kind":"app","tab":"apps","app":id}));
        }
    }
    Ok(json!({"items":items,"sampled":true,"perKindLimit":3}))
}
