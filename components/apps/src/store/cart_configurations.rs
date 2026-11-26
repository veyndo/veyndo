//! Cart configurations stay bound to the package, rule revision and price they were made with.
use crate::domain::configuration_inputs;
use crate::service::configuration_runtime::{configured_contribution, prepare};
use crate::store::packages::package;
use crate::store::records;
use std::collections::HashMap;
use veyndo_app_model::manifest::Manifest;
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::{Result, State, bad, conflict, hash};
use veyndo_shop::types::cart::StoredCart;

/// Rechecks every configuration of a cart against the current package, rules and price, inside the cart transaction.
pub async fn validate_configurations(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    prepared: &HashMap<String, std::sync::Arc<Sandbox>>,
) -> Result<()> {
    crate::store::locks::tenant_scope(tx, &c.tenant).await?;
    for config in c
        .data
        .app_configurations
        .values()
        .filter(|v| c.data.items.iter().any(|i| i.id == v.product_id))
    {
        let row = sqlx::query!(
            "SELECT manifest,version FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
            c.tenant,
            config.app
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(conflict("Configuration app unavailable"))?;
        let m: Manifest =
            serde_json::from_value(row.manifest).map_err(|_| bad("Invalid package"))?;
        let contract = m
            .configuration
            .as_ref()
            .ok_or(conflict("Upgrade and reconfigure this app"))?;
        if contract.currency != config.currency
            || m.version != config.app_version
            || contract.entity != config.entity
            || configuration_inputs::record(contract, &config.fields)? != config.record
        {
            return Err(conflict("App changed; configure again"));
        }
        let r = records::get(
            tx,
            &c.tenant,
            &config.app,
            &contract.entity,
            &config.record,
            false,
        )
        .await?
        .ok_or(conflict("Configuration rules missing"))?;
        let fee = configured_contribution(
            prepared
                .get(&hash(&contract.wasm_source))
                .cloned()
                .ok_or(conflict("Cart app modules changed; retry checkout"))?,
            contract,
            r[&contract.price_field]
                .as_i64()
                .ok_or(bad("App price missing"))?,
            &config.fields,
        )
        .await?;
        if records::RecordMeta::of(&r)
            .revision
            .ok_or(bad("App revision missing"))?
            != config.rule_revision
            || fee != config.fee_minor
        {
            return Err(conflict("App price changed; configure again"));
        }
    }
    Ok(())
}
/// Compile before opening checkout locks. Strong Arc references survive cache eviction;
/// the locked validation above checks the exact current source digest again.
pub async fn prepare_configurations(
    a: &State,
    c: &StoredCart,
) -> Result<HashMap<String, std::sync::Arc<Sandbox>>> {
    let mut prepared = HashMap::new();
    for config in c.data.app_configurations.values() {
        let m = package(a, &c.tenant, &config.app, true).await?;
        let contract = m
            .configuration
            .as_ref()
            .ok_or(conflict("App configuration missing"))?;
        let digest = hash(&contract.wasm_source);
        if let std::collections::hash_map::Entry::Vacant(e) = prepared.entry(digest) {
            e.insert(prepare(contract).await?);
        }
    }
    Ok(prepared)
}

/// Locks a cart by its token for a configuration change; older stored shapes are upgraded on the way.
pub async fn lock_cart(conn: &mut sqlx::PgConnection, t: &str, token: &str) -> Result<StoredCart> {
    veyndo_checkout::store::carts::lock_by_token(conn, t, token)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?
        .into_cart()
}

/// Stores the changed document and revision of a locked cart.
pub async fn save_cart(conn: &mut sqlx::PgConnection, c: &StoredCart) -> Result<()> {
    veyndo_checkout::store::app_carts::save(conn, &c.id, &serde_json::json!(c.data), c.revision)
        .await
}
