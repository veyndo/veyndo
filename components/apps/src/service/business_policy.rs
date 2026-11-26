//! The tenant business policy: a sandboxed Wasm module for company purchase approval, activated by the merchant, persisted with its digest and compiled on the bounded cache.
use crate::service::sandbox_cache::Cache;
use crate::store::business_policy as stored;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash};
use veyndo_identity::merchant;

/// The JSON fields `activate` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ActivateInput {
    #[serde(deserialize_with = "lenient")]
    wat: Option<String>,
}

/// Compiles, approves and stores the merchant's policy module; the cache serves it at once.
pub async fn activate(a: &State, cache: &Cache, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: ActivateInput = view(v);
    let t = merchant(h)?;
    let wat = input.wat.as_deref().ok_or(bad("wat required"))?.to_string();
    let source = wat.clone();
    let compiled = tokio::task::spawn_blocking(move || Sandbox::new(&source))
        .await
        .map_err(|e| bad(e.to_string()))?
        .map_err(bad)?;
    compiled.approve(0, 100_000).map_err(bad)?;
    let digest = hash(&wat);
    stored::save(a, &t, &wat, &digest).await?;
    cache.insert(t, Arc::new(compiled));
    Ok(
        json!({"activated":true,"digest":digest,"hook":"company.purchase.approve","fuel":10000,"memoryBytes":1048576,"hostImports":false}),
    )
}

/// The persisted policy digest and revision, or the built-in policy.
pub async fn state(a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    let r = stored::state(a, &t).await?;
    Ok(match r {
        Some(r) => {
            json!({"digest":r.digest,"revision":r.revision,"source":"persisted"})
        }
        None => json!({"source":"built-in"}),
    })
}

/// The compiled policy of a tenant; a stored source that no longer matches its digest is refused.
pub async fn prepare(a: &State, cache: &Cache, tenant: &str) -> Result<Arc<Sandbox>> {
    let row = stored::source(a, tenant)
        .await?
        .ok_or(bad("Business approval policy missing"))?;
    let digest = row.digest;
    if let Some(cached) = cache.get(tenant, &digest) {
        return Ok(cached);
    }
    let source = row.wat;
    if hash(&source) != digest {
        return Err(conflict(
            "Business policy digest mismatch; ask the merchant to reactivate its policy",
        ));
    }
    cache.compile(tenant, source).await
}
