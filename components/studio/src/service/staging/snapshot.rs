//! Fixed publishable units: product content/translations, settings, experience and app packages.
use crate::store::staging_clone::scope_app_records;
use crate::store::staging_snapshot as db;
use crate::store::{
    staging_assets as assets, staging_categories as categories, staging_documents as documents,
};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::domain::core_extensions::configuration;
use veyndo_app_platform::store::records::rows as record_rows;
use veyndo_b2b::store::staging as b2b;
use veyndo_catalog::service::product_identity::assign_categories;
use veyndo_core::document::{lenient, merge, view};
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
        let id = view::<Identified>(&value).id.unwrap();
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
                    format!(
                        "appdata:{app}:{}:{}",
                        e.name,
                        view::<Identified>(&data).id.unwrap()
                    ),
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
/// A document's string ID.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Identified {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

/// The fields of a staged product document that publication reads; the full document feeds the column allowlist.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StagedProduct {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    category: Option<String>,
    #[serde(deserialize_with = "lenient")]
    description: Option<String>,
    #[serde(deserialize_with = "lenient")]
    price: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    tax_rate: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    product_number: Option<String>,
    #[serde(rename = "categoryIds", deserialize_with = "lenient")]
    category_ids: Option<Vec<Value>>,
    #[serde(rename = "channelVisibility", deserialize_with = "lenient")]
    channel_visibility: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    translations: Option<Vec<Value>>,
}

/// A staged channel visibility entry.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StagedChannel {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    visible: Option<bool>,
}

/// A staged product translation.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StagedTranslation {
    #[serde(deserialize_with = "lenient")]
    language_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    description: Option<String>,
}

pub async fn product_content(tx: &mut Tx<'_>, t: &str, value: &Value) -> Result<()> {
    let mut published = value.clone();
    live_asset_urls(&mut published, t);
    let value = &published;
    let staged: StagedProduct = view(value);
    let id = staged.id.as_deref().ok_or(bad("Product ID required"))?;
    // A new staged product enters live with zero inventory; stock remains an independent operational write.
    db::insert_product(
        tx,
        t,
        id,
        &db::NewProduct {
            name: staged.name.as_deref(),
            category: staged.category.as_deref(),
            description: staged.description.as_deref(),
            price: staged.price,
            tax_rate: staged.tax_rate,
            active: staged.active.unwrap_or(true),
            product_number: staged.product_number.as_deref(),
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
    if let Some(ids) = &staged.category_ids {
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
    if let Some(channels) = &staged.channel_visibility {
        db::clear_channel_visibility(tx, t, id).await?;
        for channel in channels {
            let channel: StagedChannel = view(channel);
            db::insert_channel_visibility(tx, t, id, channel.id.as_deref(), channel.visible)
                .await?;
        }
    }
    for tr in staged
        .translations
        .as_ref()
        .ok_or(bad("Translations required"))?
    {
        let tr: StagedTranslation = view(tr);
        db::upsert_translation(
            tx,
            t,
            id,
            tr.language_id.as_deref(),
            tr.name.as_deref(),
            tr.description.as_deref(),
        )
        .await?;
    }
    let stored = crate::store::products::get(tx, t, id).await?;
    sync_product(tx, t, &json!(stored)).await?;
    veyndo_core::store::outbox::publish_on(
        &mut **tx,
        t,
        "product.published",
        &json!({"productId":id,"revision":stored.revision}),
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
    let id = view::<Identified>(&value).id.unwrap();
    let trs = db::translations(tx, t, &id).await?;
    let translations = trs
        .into_iter()
        .map(|(language_id, name, description)| json!({"language_id":language_id,"name":name,"description":description}))
        .collect::<Vec<_>>();
    let category_ids = db::category_ids(tx, t, &id).await?;
    let channel_visibility = db::channel_visibility(tx, t, &id)
        .await?
        .into_iter()
        .map(|(id, visible)| json!({"id":id,"visible":visible}))
        .collect::<Vec<_>>();
    merge(
        &mut value,
        json!({"translations":translations,"categoryIds":category_ids,"channelVisibility":channel_visibility}),
    )?;
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
