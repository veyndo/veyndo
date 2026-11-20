//! Generic pure-Wasm contribution executor; the installed package supplies all business predicates.
use crate::domain::configuration_inputs;
use crate::service::sandbox_cache::Cache;
use serde_json::Value;
use std::sync::Arc;
use std::sync::OnceLock;
use veyndo_app_model::configuration::Contract;
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::{Result, bad, conflict, hash};
/// The compiled guest of a configuration contract.
pub async fn prepare(c: &Contract) -> Result<Arc<Sandbox>> {
    prepare_source(c.wasm_source.clone()).await
}
/// The compiled guest of a module source.
pub async fn prepare_source(source: String) -> Result<Arc<Sandbox>> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE
        .get_or_init(Cache::default)
        .compile(&hash(&source), source)
        .await
}
/// Runs the guest once to check that a fee is acceptable to it.
pub async fn validate_fee(c: &Contract, fee: i64) -> Result<i64> {
    run(
        prepare(c).await?,
        fee,
        None,
        c.inputs.as_ref().map_or(1, |inputs| inputs.arguments.len()),
    )
    .await
}
/// The fee the guest computes for accepted inputs; the module must be the prepared one.
pub async fn configured_contribution(
    guest: Arc<Sandbox>,
    c: &Contract,
    fee: i64,
    fields: &Value,
) -> Result<i64> {
    if guest.digest() != hash(&c.wasm_source) {
        return Err(conflict("App module changed; retry checkout"));
    }
    let args = configuration_inputs::arguments(c, fields)?;
    run(
        guest,
        fee,
        Some(args),
        c.inputs.as_ref().map_or(1, |inputs| inputs.arguments.len()),
    )
    .await
}
async fn run(
    guest: Arc<Sandbox>,
    fee: i64,
    arguments: Option<Vec<i64>>,
    arity: usize,
) -> Result<i64> {
    tokio::task::spawn_blocking(move || {
        veyndo_apps::configuration::run(&guest, fee, arguments, arity).map_err(bad)
    })
    .await
    .map_err(|_| bad("Configuration runtime failed"))?
}
