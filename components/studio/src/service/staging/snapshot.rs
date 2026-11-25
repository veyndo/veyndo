//! Fixed publishable units: product content/translations, settings, experience and app packages.
use crate::store::staging_clone::scope_app_records;
use crate::store::staging_snapshot as db;
use crate::store::{
    staging_assets as assets, staging_categories as categories, staging_documents as documents,
};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::domain::core_extensions::configuration;
use veyndo_app_platform::store::records::rows as record_rows;
use veyndo_b2b::store::staging as b2b;
use veyndo_catalog::service::product_identity::assign_categories;
use veyndo_core::{Result, bad, conflict};
use veyndo_intelligence::knowledge::sync_product;
use veyndo_orders::domain::order_machine::default_machine;
use veyndo_orders::store::company_release as company;
use veyndo_shop::service::settings_writes::snapshot_scopes;
/// The transaction every staging step shares.
pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;
pub async fn snapshot(tx: &mut Tx<'_>, t: &str) -> Result<Value> {
    let mut result = serde_json::Map::new();
    // Same settings-before-purchasing-policy lock order as native organization writes.
    db::lock_settings(tx, t).await?;
    b2b::snapshot_configuration(tx, t, &mut result).await?;
    assets::snapshot_assets(tx, t, &mut result).await?;
    company::snapshot(tx, t, &mut result).await?;
    snapshot_scopes(tx, t, &mut result).await?;
    for value in db::products(tx, t).await? {
        let id = value["id"].as_str().unwrap().to_string();
        result.insert(
            format!("product:{id}"),
            product_projection(tx, t, value).await?,
        );
    }
    categories::snapshot(tx, t, &mut result).await?;
    if let Some(data) = db::settings(tx, t).await? {
        result.insert("settings".into(), data);
    }
    if let Some(data) = db::experience(tx, t).await? {
        result.insert("experience".into(), data);
    }
    for package in db::packages(tx, t).await? {
        let app = package.id;
        let manifest = package.manifest;
        let m: Manifest =
            serde_json::from_value(manifest.clone()).map_err(|_| bad("Invalid app"))?;
        result.insert(
            format!("app:{app}"),
            json!({"manifest":manifest,"active":package.active}),
        );
        scope_app_records(tx, t).await?;
        for e in m.entities.iter().filter(|e| {
            configuration(e)
                && !m
                    .actions
                    .iter()
                    .any(|a| a.entity.as_deref() == Some(e.name.as_str()) && a.handler == "submit")
        }) {
            let records = record_rows(tx, t, &app, e, 1001).await?;
            if records.len() > 1000 {
                return Err(bad(
                    "Prototype release supports at most 1000 records per app entity",
                ));
            }
            for r in records {
                let mut data = r;
                data.as_object_mut().unwrap().remove("revision");
                result.insert(
                    format!("appdata:{app}:{}:{}", e.name, data["id"].as_str().unwrap()),
                    data,
                );
            }
        }
    }
    for kind in ["rule", "promotion", "flow", "channel"] {
        for (id, data) in db::units(tx, t, kind).await? {
            result.insert(format!("{kind}:{id}"), data);
        }
    }
    let data = db::order_workflow(tx, t).await?;
    result.insert(
        "order-workflow".into(),
        data.unwrap_or_else(|| json!(default_machine())),
    );
    documents::snapshot(tx, t, &mut result).await?;
    Ok(Value::Object(result))
}
pub async fn product_content(tx: &mut Tx<'_>, t: &str, value: &Value) -> Result<()> {
    let mut published = value.clone();
    live_asset_urls(&mut published, t);
    let value = &published;
    let id = value["id"].as_str().ok_or(bad("Product ID required"))?;
    // A new staged product enters live with zero inventory; stock remains an independent operational write.
    db::insert_product(
        tx,
        t,
        id,
        &db::NewProduct {
            name: value["name"].as_str(),
            category: value["category"].as_str(),
            description: value["description"].as_str(),
            price: value["price"].as_f64(),
            tax_rate: value["tax_rate"].as_f64(),
            active: value["active"].as_bool().unwrap_or(true),
            product_number: value["product_number"].as_str(),
        },
    )
    .await?;
    // Explicit allowlist preserves independently changing stock and never copies tenant keys.
    let n = db::update_product_content(tx, t, id, value).await?;
    if n != 1 {
        return Err(conflict(
            "Product creation/deletion needs an explicit catalog migration",
        ));
    }
    if let Some(ids) = value["categoryIds"].as_array() {
        assign_categories(
            tx,
            t,
            id,
            &ids.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    if let Some(channels) = value["channelVisibility"].as_array() {
        db::clear_channel_visibility(tx, t, id).await?;
        for channel in channels {
            db::insert_channel_visibility(
                tx,
                t,
                id,
                channel["id"].as_str(),
                channel["visible"].as_bool(),
            )
            .await?;
        }
    }
    for tr in value["translations"]
        .as_array()
        .ok_or(bad("Translations required"))?
    {
        db::upsert_translation(
            tx,
            t,
            id,
            tr["language_id"].as_str(),
            tr["name"].as_str(),
            tr["description"].as_str(),
        )
        .await?;
    }
    let published = crate::store::products::get(tx, t, id).await?;
    sync_product(tx, t, &json!(published)).await?;
    veyndo_core::store::outbox::publish_on(
        &mut **tx,
        t,
        "product.published",
        &json!({"productId":id,"revision":published.revision}),
    )
    .await?;
    Ok(())
}

/// Local asset IDs clone unchanged; their delivery tenant changes from private staging to live.
fn live_asset_urls(value: &mut Value, tenant: &str) {
    match value {
        Value::String(s) if s.starts_with("/store-api/assets/") => {
            if let Ok(mut url) = reqwest::Url::parse(&format!("https://local{s}")) {
                let pairs = url
                    .query_pairs()
                    .filter(|(k, _)| k != "shop")
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect::<Vec<_>>();
                url.set_query(None);
                url.query_pairs_mut()
                    .extend_pairs(pairs)
                    .append_pair("shop", tenant);
                *s = format!("{}?{}", url.path(), url.query().unwrap_or_default());
            }
        }
        Value::Array(items) => {
            for item in items {
                live_asset_urls(item, tenant);
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                live_asset_urls(item, tenant);
            }
        }
        _ => {}
    }
}
/// Exact selected-product readback includes database defaults and the live asset delivery URLs, without a whole-shop rescan.
pub async fn product_snapshot(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Value> {
    let value = db::product(tx, t, id).await?;
    product_projection(tx, t, value).await
}
async fn product_projection(tx: &mut Tx<'_>, t: &str, mut value: Value) -> Result<Value> {
    let id = value["id"].as_str().unwrap().to_string();
    let trs = db::translations(tx, t, &id).await?;
    value["translations"] = json!(
        trs.into_iter()
            .map(|(language_id, name, description)| json!({"language_id":language_id,"name":name,"description":description}))
            .collect::<Vec<_>>()
    );
    value["categoryIds"] = json!(db::category_ids(tx, t, &id).await?);
    value["channelVisibility"] = json!(
        db::channel_visibility(tx, t, &id)
            .await?
            .into_iter()
            .map(|(id, visible)| json!({"id":id,"visible":visible}))
            .collect::<Vec<_>>()
    );
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_rebinds_only_local_asset_urls() {
        let mut value = json!({"media":[{"url":"/store-api/assets/image?shop=stage"}],"external":"https://example.test/?shop=stage"});
        live_asset_urls(&mut value, "live");
        assert_eq!(
            value["media"][0]["url"],
            "/store-api/assets/image?shop=live"
        );
        assert_eq!(value["external"], "https://example.test/?shop=stage");
    }
}
