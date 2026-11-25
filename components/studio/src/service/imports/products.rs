//! Private native product intake delegates all catalogue validation, history and selected release to the existing owner.
use super::SourceRecord;
use crate::http::product_editor::create_product;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
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
    if let Some(extra) =
        sqlx::query_scalar::<_, Value>("SELECT extra FROM products WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(&id)
            .fetch_optional(&a.db)
            .await?
    {
        if extra["identity"]["migrationDigest"] == digest {
            return Ok(json!({"id":id,"replayed":true}));
        }
        return Err(conflict(
            "Source product already has a different reviewed snapshot; prepare an explicit update",
        ));
    }
    let payload = mapped(a, &t, r, options).await?;
    let Json(result) = create_product(Shared(a.clone()), h.clone(), Json(payload)).await?;
    Ok(json!({"id":result["id"],"replayed":false,"stockActivated":false}))
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
    let amount = &r.data["price"];
    let code = amount["currency"]
        .as_str()
        .ok_or(bad("Source price currency required"))?;
    let minor = amount["minor"]
        .as_i64()
        .filter(|n| *n >= 0)
        .ok_or(bad("Exact source price required"))?;
    let scale = amount["scale"]
        .as_u64()
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
        price = veyndo_pricing::pricing::calculate(&veyndo_pricing::pricing::PriceInput {
            price,
            tax_rate: tax,
            gross: true,
            calculated: false,
            decimals: scale.into(),
            interval: 10_f64.powi(-i32::from(scale)),
            ..Default::default()
        })
        .unit_price;
        if !price.is_finite() || price * 10_f64.powi(scale.into()) > 9_007_199_254_740_991.0 {
            return Err(bad(
                "Converted source price exceeds exact native compatibility range",
            ));
        }
    }
    let names = r.data["name"]
        .as_object()
        .ok_or(bad("Translated source names required"))?;
    let translations: serde_json::Map<String, Value> = names
        .iter()
        .map(|(locale, name)| {
            (
                settings.languages().key(locale),
                json!({"name":name,"description":r.data["description"][locale]}),
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
    let sku = r.data["sku"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("IMPORT-{id}"));
    let relations = super::relations::product(r, &namespace)?;
    let mut payload = json!({"id":id,"revision":0,"settingsRevision":revision,"translations":translations,"extra":{"priceCurrency":code,"identity":{"migrationDigest":digest,"importSource":{"system":r.source,"shop":r.source_shop,"id":r.source_id},"sourceRecordVersion":r.version,"sourceRecordDigest":hash(&json!(r).to_string()),"sourceStock":r.data["stock"].as_i64(),"sourcePriceBasis":basis,"sourcePrice":amount}},"catalog":{"active":active,"productNumber":sku,"categoryIds":relations["categoryIds"],"parentId":relations["parentId"],"options":relations["options"]},"commerce":{"price":price,"taxRate":tax,"stock":0,"minPurchase":1,"purchaseSteps":1,"maxPurchase":null,"deliveryDays":0,"listPrice":null,"advancedPrices":[],"media":[],"properties":{}}});
    if !rich.is_empty() {
        payload["extra"]["richDescription"] = json!(rich);
    }
    Ok(payload)
}
