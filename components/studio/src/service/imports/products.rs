//! Private native product intake delegates all catalogue validation, history and selected release to the existing owner.
use super::SourceRecord;
use crate::http::product_editor::create_product;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::config;
pub async fn import(
    a: &State,
    h: &RequestContext,
    r: &SourceRecord,
    options: &Value,
) -> Result<Value> {
    permit(h, "catalog.write")?;
    let t = merchant(h)?;
    if parent(a, &t).await?.is_none() {
        return Err(bad(
            "Product migration requires a private preview; live inventory was not changed",
        ));
    }
    let namespace = super::identity_namespace(a, &t).await?;
    let id = r.identity(&namespace);
    let digest = hash(&json!({"record":r,"options":options}).to_string());
    if let Some(extra) = crate::store::imports::product_extra(a, &t, &id).await? {
        if view::<Migrated>(&extra).identity.migration_digest == digest {
            return Ok(json!({"id":id,"replayed":true}));
        }
        return Err(conflict(
            "Source product already has a different reviewed snapshot; prepare an explicit update",
        ));
    }
    let payload = mapped(a, &t, r, options).await?;
    let Json(result) = create_product(Shared(a.clone()), h.clone(), Json(payload)).await?;
    Ok(json!({"id":view::<Created>(&result).id,"replayed":false,"stockActivated":false}))
}

/// A stored product's migration identity.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Migrated {
    #[serde(deserialize_with = "lenient")]
    identity: MigrationIdentity,
}
/// The digest of the reviewed source snapshot a product was migrated from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MigrationIdentity {
    migration_digest: Value,
}
/// The ID of a created product.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Created {
    id: Value,
}
/// The reviewed fields of a source product.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProductSource {
    price: Value,
    #[serde(deserialize_with = "lenient")]
    name: Option<serde_json::Map<String, Value>>,
    description: Value,
    #[serde(deserialize_with = "lenient")]
    sku: Option<String>,
    #[serde(deserialize_with = "lenient")]
    stock: Option<i64>,
}
/// A product's native catalogue relations.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Relations {
    category_ids: Value,
    parent_id: Value,
    options: Value,
}
/// The JSON fields `mapped` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MappedInput {
    #[serde(deserialize_with = "lenient")]
    tax_rate: Option<f64>,
    source_price_basis: Value,
}

/// Shared native mapping for writes and read-only reconciliation; no second price/translation calculator.
pub async fn mapped(a: &State, t: &str, r: &SourceRecord, options: &Value) -> Result<Value> {
    let input: MappedInput = view(options);
    let active = match r.data.get("catalogActive") {
        None | Some(Value::Null) => true,
        Some(value) => value
            .as_bool()
            .ok_or(bad("Source catalogue visibility must be boolean"))?,
    };
    let namespace = super::identity_namespace(a, t).await?;
    let id = r.identity(&namespace);
    let digest = hash(&json!({"record":r,"options":options}).to_string());
    let (settings, revision) = config(a, t).await?;
    let tax = input
        .tax_rate
        .filter(|n| n.is_finite() && (0.0..=50.0).contains(n))
        .ok_or(bad("Explicitly review the native tax rate"))?;
    let source: ProductSource = view(&r.data);
    let amount = &source.price;
    let price_source: super::SourceAmount = view(amount);
    let code = price_source
        .currency
        .as_deref()
        .ok_or(bad("Source price currency required"))?;
    let minor = price_source
        .minor
        .filter(|n| *n >= 0)
        .ok_or(bad("Exact source price required"))?;
    let scale = price_source
        .scale
        .and_then(|n| u8::try_from(n).ok())
        .ok_or(bad("Currency scale required"))?;
    let currency = settings.currencies.selected(code)?;
    if currency.scale != scale {
        return Err(bad("Source currency scale differs from native currency"));
    }
    let mut price = veyndo_pricing::money::Money::new(
        minor,
        veyndo_pricing::money::Currency::new(code, scale)
            .map_err(|_| bad("Invalid source currency"))?,
    )
    .decimal()
    .parse::<f64>()
    .map_err(|_| bad("Price is outside native compatibility range"))?;
    if !price.is_finite() || price * 10_f64.powi(scale.into()) > 9_007_199_254_740_991.0 {
        return Err(bad("Source price exceeds exact native compatibility range"));
    }
    let basis = input.source_price_basis.as_str().unwrap_or("gross");
    if let Some(source) = r.data.get("priceBasis").filter(|value| !value.is_null())
        && (!source
            .as_str()
            .is_some_and(|value| ["gross", "net"].contains(&value))
            || input.source_price_basis != *source)
    {
        return Err(bad("Explicitly confirm the declared source price basis"));
    }
    if basis == "net" {
        use veyndo_pricing::binary::Binary;
        let wire = |value| Binary::from_wire(value).map_err(bad);
        price = veyndo_pricing::pricing::calculate(&veyndo_pricing::pricing::PriceInput {
            price: wire(price)?,
            tax_rate: wire(tax)?,
            gross: true,
            calculated: false,
            decimals: scale.into(),
            interval: Binary::from_decimal(
                veyndo_pricing::decimal::Decimal::new(1, scale.into()).map_err(bad)?,
            )
            .map_err(bad)?,
            ..Default::default()
        })
        .map_err(bad)?
        .unit_price
        .decimal()
        .to_wire()
        .map_err(bad)?;
        if !price.is_finite() || price * 10_f64.powi(scale.into()) > 9_007_199_254_740_991.0 {
            return Err(bad(
                "Converted source price exceeds exact native compatibility range",
            ));
        }
    }
    let names = source
        .name
        .as_ref()
        .ok_or(bad("Translated source names required"))?;
    let translations: serde_json::Map<String, Value> = names
        .iter()
        .map(|(locale, name)| {
            (
                settings.languages().key(locale),
                json!({"name":name,"description":source.description.get(locale).cloned().unwrap_or(Value::Null)}),
            )
        })
        .collect();
    let mut rich = serde_json::Map::new();
    if let Some(value) = r.data.get("richDescription") {
        let values = value
            .as_object()
            .ok_or(bad("Translated native rich descriptions required"))?;
        for (locale, blocks) in values {
            rich.insert(settings.languages().key(locale), blocks.clone());
        }
    }
    let sku = source
        .sku
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("IMPORT-{id}"));
    let relations: Relations = view(&super::relations::product(r, &namespace)?);
    let mut extra = json!({"priceCurrency":code,"identity":{"migrationDigest":digest,"importSource":{"system":r.source,"shop":r.source_shop,"id":r.source_id},"sourceRecordVersion":r.version,"sourceRecordDigest":hash(&json!(r).to_string()),"sourceStock":source.stock,"sourcePriceBasis":basis,"sourcePrice":amount}});
    if !rich.is_empty() {
        merge(&mut extra, json!({ "richDescription": rich }))?;
    }
    let payload = json!({"id":id,"revision":0,"settingsRevision":revision,"translations":translations,"extra":extra,"catalog":{"active":active,"productNumber":sku,"categoryIds":relations.category_ids,"parentId":relations.parent_id,"options":relations.options},"commerce":{"price":price,"taxRate":tax,"stock":0,"minPurchase":1,"purchaseSteps":1,"maxPurchase":null,"deliveryDays":0,"listPrice":null,"advancedPrices":[],"media":[],"properties":{}}});
    Ok(payload)
}
