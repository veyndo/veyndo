//! Private cloned shops: clone, snapshot, reviewed diff and selective atomic release of changes. The server lends app installation, record saves, flow checks and settings guards through the release port.
use crate::domain::staging_access as access;
use crate::store::staging_release as db;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use veyndo_core::document::{merge, view};
use veyndo_core::store::environments::{owned_stage as owned_on, parent};
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_identity::merchant;
use veyndo_orders::store::company::lock as lock_company;
mod clone;
mod final_config;
mod products;
mod release;
mod snapshot;
pub use clone::{create, create_preview};
pub use release::release;
pub use snapshot::{Tx, product_content, snapshot};

/// The sandbox baseline: per release unit, the staged value and the live value at the last clone or release.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Baseline {
    pub stage: Value,
    pub live: Value,
    /// Any other stored field passes through unchanged.
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

impl Baseline {
    /// Records the staged and live value of a unit; a missing side becomes an object, as indexed assignment did.
    pub fn record(&mut self, key: &str, stage: Option<Value>, live: Option<Value>) -> Result<()> {
        if let Some(value) = stage {
            merge(&mut self.stage, json!({ key: value }))?;
        }
        if let Some(value) = live {
            merge(&mut self.live, json!({ key: value }))?;
        }
        Ok(())
    }
}

/// The caller's live shop; environment management refuses a sandbox.
pub async fn live(a: &State, h: &RequestContext) -> Result<String> {
    let t = merchant(h)?;
    if parent(a, &t).await?.is_some() {
        return Err(bad("Use the live workspace for environment management"));
    }
    Ok(t)
}
/// A sandbox of the live shop with its baseline.
pub async fn owned(a: &State, live: &str, stage: &str) -> Result<Value> {
    owned_on(&a.db, live, stage).await
}
/// Transactional callers retain their current backend and the same live/stage ownership guard.
/// The live shop's environments and recent releases.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = live(&a, &h).await?;
    let rows = db::environments(&a, &t).await?;
    let history = db::releases(&a, &t).await?;
    Ok(Json(
        json!({"live":t,"environments":rows.iter().map(|r|json!({"id":r.tenant,"name":r.name,"createdAt":r.created_at})).collect::<Vec<_>>(),"releases":history.iter().map(|r|json!({"id":r.id,"environment":r.environment,"selections":r.selections,"createdAt":r.created_at})).collect::<Vec<_>>() }),
    ))
}
/// The readable changes of a sandbox against its baseline, with live conflicts.
pub async fn diff(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = live(&a, &h).await?;
    let base: Baseline = view(&owned(&a, &t, &id).await?);
    let mut tx = a.db.begin().await?;
    lock_company(&mut tx, &t).await?;
    lock_company(&mut tx, &id).await?;
    let current = snapshot(&mut tx, &id).await?;
    let actual = snapshot(&mut tx, &t).await?;
    let mut changes = vec![];
    for (key, value) in current.as_object().unwrap() {
        if !access::can_read_unit(&h, key, &current)? {
            continue;
        }
        if base.stage.get(key) != Some(value) {
            changes.push(json!({"key":key,"before":access::before(&h,key,&base.stage,&current),"after":value,"digest":hash(&value.to_string()),"conflict":actual.get(key)!=base.live.get(key)}));
        }
    }
    Ok(Json(
        json!({"environment":id,"mainLocale":current.get("settings").and_then(|settings| settings.get("mainLocale")),"changes":changes,"excluded":["inventory","customers","orders","payments","credentials","observations","b2b-employees","b2b-spending","b2b-approvals","b2b-quotes","b2b-lists"]}),
    ))
}

#[cfg(test)]
mod tests {
    use super::Baseline;
    use serde_json::json;
    use veyndo_core::document::view;

    #[test]
    fn baseline_records_keep_the_indexed_shape() {
        for stored in [
            json!({"stage":{"settings":{"a":1}},"live":{"settings":{"a":0}}}),
            json!({"stage":{},"live":{},"note":"kept"}),
            json!({}),
        ] {
            let mut old = stored.clone();
            old["stage"]["channel:web"] = json!({"kind":"storefront"});
            old["live"]["channel:web"] = json!({"kind":"pos"});
            old["live"]["settings-channel:pos"] = json!({"x":1});
            let mut new: Baseline = view(&stored);
            new.record(
                "channel:web",
                Some(json!({"kind":"storefront"})),
                Some(json!({"kind":"pos"})),
            )
            .unwrap();
            new.record("settings-channel:pos", None, Some(json!({"x":1})))
                .unwrap();
            assert_eq!(json!(new), old);
        }
    }
}
