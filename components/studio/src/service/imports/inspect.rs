//! Read-only native import reconciliation compares mapped current values, never a successful replay flag.
use super::inspect_history as history;
use super::inspect_media as media;
use super::inspect_product as product;
use super::{SourceRecord, categories, coupons, identity_namespace, products, schema};
use crate::http::product_editor::product_editor;
use crate::ports::ImportHost;
use crate::store::imports as db;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::model_contract::validate_native;
use veyndo_core::{RequestContext, Result, StatusCode, bad, hash};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::config;
/// A source record to compare and its import options.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InspectRequest {
    record: Value,
    options: Value,
    #[serde(deserialize_with = "lenient")]
    details: Option<bool>,
}

/// The source-owned fields of a mapped product payload.
#[derive(Deserialize, Default)]
#[serde(default)]
struct MappedProduct {
    catalog: Value,
    translations: Value,
    #[serde(deserialize_with = "lenient")]
    extra: MappedExtra,
    #[serde(deserialize_with = "lenient")]
    commerce: MappedCommerce,
}
/// A mapped product's currency, migration identity and optional rich description.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MappedExtra {
    price_currency: Value,
    #[serde(deserialize_with = "lenient")]
    identity: MappedIdentity,
    #[serde(deserialize_with = "present")]
    rich_description: Option<Value>,
}
/// A mapped product's migration digest.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MappedIdentity {
    migration_digest: Value,
}
/// A mapped product's price and tax rate.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MappedCommerce {
    price: Value,
    tax_rate: Value,
}
/// A mapped category.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MappedCategory {
    parent_id: Value,
    data: Value,
}
/// A source record's reviewed destination mapping.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Destination {
    destination: Value,
}
/// The reviewed digest of a page destination.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PageOptions {
    #[serde(deserialize_with = "lenient")]
    destination_digest: Option<String>,
}
/// A product document's price currency.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Priced {
    #[serde(deserialize_with = "lenient")]
    extra: PriceCurrency,
}
/// The currency key of a product's extra fields.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PriceCurrency {
    #[serde(deserialize_with = "lenient")]
    price_currency: Option<String>,
}
/// A present field, including an explicit null.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

pub async fn inspect<H: ImportHost>(host: &H, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: InspectRequest = view(v);
    let a = host.core();
    let t = merchant(h)?;
    permit(h, "apps.manage")?;
    validate_native(&schema("merchant.import.inspect").unwrap(), v)?;
    let r: SourceRecord =
        serde_json::from_value(input.record.clone()).map_err(|_| bad("Invalid source record"))?;
    r.validate()?;
    let namespace = identity_namespace(a, &t).await?;
    let mut id = r.identity(&namespace);
    let (expected, actual) = match r.kind.as_str() {
        "products" => {
            permit(h, "catalog.read")?;
            let mapped = products::mapped(a, &t, &r, &input.options).await?;
            let actual = match product_editor(Shared(a.clone()), h.clone(), Path(id.clone())).await
            {
                Ok(Json(value)) => Some(value),
                Err(e) if e.0 == StatusCode::NOT_FOUND => None,
                Err(e) => return Err(e),
            };
            // Inventory, new merchant metadata and later media have independent native owners.
            // Only source-owned fields are asserted by this product record; assets get their own comparison.
            let mapped: MappedProduct = view(&mapped);
            let mut catalog = mapped.catalog;
            if let Some(categories) = catalog.get_mut("categoryIds").and_then(Value::as_array_mut) {
                categories.sort_by_key(Value::to_string);
            }
            let mut translations = mapped.translations;
            let settings = config(a, &t).await?.0;
            let main = settings.languages().key(&settings.main_locale);
            if let Some(text) = translations.get_mut(&main).filter(|text| text.is_object())
                && text.get("description").is_none_or(Value::is_null)
            {
                merge(text, json!({ "description": "" }))?;
            }
            let mut extra = json!({"priceCurrency":mapped.extra.price_currency,"identity":{"migrationDigest":mapped.extra.identity.migration_digest}});
            if let Some(rich) = mapped.extra.rich_description {
                merge(&mut extra, json!({ "richDescription": rich }))?;
            }
            let expected = json!({"translations":translations,"catalog":catalog,"extra":extra,"commerce":{"price":mapped.commerce.price,"taxRate":mapped.commerce.tax_rate}});
            (expected, actual)
        }
        "categories" => {
            permit(h, "catalog.read")?;
            let mapped: MappedCategory =
                view(&categories::mapped(a, &t, &r, &input.options).await?);
            let actual = db::category(a, &t, &id).await?;
            (
                json!({"parentId":mapped.parent_id,"data":mapped.data}),
                actual,
            )
        }
        "assets" => media::inspect(a, h, &r, &namespace, &id).await?,
        "pages" => {
            host.inspect_record(
                h,
                &id,
                &view::<Destination>(&r.data).destination,
                view::<PageOptions>(&input.options)
                    .destination_digest
                    .as_deref()
                    .ok_or(bad("Reviewed destination digest required"))?,
            )
            .await?
        }
        "coupons" => {
            permit(h, "automation.read")?;
            id = coupons::configuration_id(&r.identity(&t));
            let actual = db::promotion(a, &t, &id).await?;
            let mut expected = view::<Destination>(&r.data).destination;
            if !expected.is_object() {
                return Err(bad("Reviewed coupon mapping required"));
            }
            merge(
                &mut expected,
                json!({ "sourceImport": {"recordDigest":hash(&json!(r).to_string())} }),
            )?;
            (expected, actual)
        }
        "customers" | "orders" | "reviews" => {
            history::inspect(a, h, &r, &input.options, &id).await?
        }
        _ => return Err(bad("Unsupported native inspection domain")),
    };
    let mut differences = vec![];
    if let Some(actual) = &actual {
        compare(&expected, actual, "", &mut differences);
    }
    let mut result = json!({"id":id,"kind":r.kind,"status":if actual.is_none(){"missing"}else if differences.is_empty(){"matching"}else{"changed"},"differences":differences,"checkedAt":chrono::Utc::now().to_rfc3339(),"comparison":"mapped-current-fields-v1","readOnly":true});
    if r.kind == "products" && input.details == Some(true) {
        let (settings, _) = config(a, &t).await?;
        let scale = settings
            .currencies
            .selected(
                view::<Priced>(&expected)
                    .extra
                    .price_currency
                    .as_deref()
                    .ok_or(bad("Product currency required"))?,
            )?
            .scale;
        merge(
            &mut result,
            json!({ "preview": {"expected":product::projection(&expected, scale)?,"actual":actual.as_ref().map(|value| product::projection(value, settings.currencies.selected(view::<Priced>(value).extra.price_currency.as_deref().ok_or(bad("Product currency required"))?)?.scale)).transpose()?,"limits":{"locales":16,"textCharacters":4000,"images":8},"publication":"private-preview-not-publication"} }),
        )?;
    }
    Ok(result)
}
/// A sparse expected object asserts only mapped fields. Arrays retain their meaningful order.
fn compare(expected: &Value, actual: &Value, path: &str, differences: &mut Vec<String>) {
    if differences.len() >= 100 {
        return;
    }
    if let Some(object) = expected.as_object() {
        for (key, value) in object {
            let escaped = key.replace('~', "~0").replace('/', "~1");
            compare(
                value,
                &actual[key],
                &format!("{path}/{escaped}"),
                differences,
            );
        }
    } else if expected != actual {
        differences.push(path.into());
    }
}
