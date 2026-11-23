//! Generic source-record admission; adapters own source formats, native domain owners retain every write.
use crate::ports::ImportHost;
use serde_json::{Value, json};
use veyndo_core::model_contract::validate_native;
use veyndo_core::shop_scope::parent;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};
mod assets;
mod categories;
mod coupons;
mod customers;
mod historical_orders;
mod inspect;
mod inspect_history;
mod inspect_media;
mod inspect_product;
pub use inspect::inspect;
mod products;
mod relations;
mod reviews;
pub use veyndo_orders::types::source_record::SourceRecord;
pub async fn identity_namespace(a: &State, t: &str) -> Result<String> {
    Ok(parent(a, t).await?.unwrap_or_else(|| t.into()))
}
pub fn schema(name: &str) -> Option<Value> {
    let mut schema = (matches!(name,"merchant.import.record"|"merchant.import.inspect")).then(||json!({"type":"object","properties":{"assetChunks":{"type":"array","maxItems":172,"items":{"type":"string","maxLength":65536}},"approve":{"type":"boolean"},"record":{"type":"object","properties":{"version":{"type":"integer","enum":[1]},"source":{"type":"string","minLength":1,"maxLength":200},"sourceShop":{"type":"string","minLength":1,"maxLength":200},"kind":{"type":"string","enum":["products","categories","pages","customers","orders","assets","reviews","coupons"]},"sourceId":{"type":"string","minLength":1,"maxLength":200},"data":{"type":"object"},"warnings":{"type":"array","maxItems":20,"items":{"type":"string","maxLength":2000}}},"required":["version","source","sourceShop","kind","sourceId","data","warnings"],"additionalProperties":false},"options":{"type":"object","properties":{"channelId":{"type":"string","maxLength":100},"taxRate":{"type":"number","minimum":0,"maximum":50},"sourcePriceBasis":{"type":"string","enum":["gross","net"]},"customerGroup":{"type":"string","maxLength":100},"locale":{"type":"string","maxLength":35},"destinationDigest":{"type":"string","minLength":64,"maxLength":64},"acknowledgedWarnings":{"type":"array","maxItems":20,"items":{"type":"string","maxLength":2000}}},"additionalProperties":false}},"required":["approve","record","options"],"additionalProperties":false}))
    ?;
    if name == "merchant.import.inspect" {
        schema["properties"].as_object_mut()?.remove("approve");
        schema["properties"].as_object_mut()?.remove("assetChunks");
        schema["properties"]["details"] = json!({"type":"boolean","description":"Explicit bounded product catalogue preview; default comparisons contain no values"});
        schema["required"] = json!(["record", "options"]);
    }
    Some(schema)
}
pub async fn invoke<H: ImportHost>(host: &H, h: &RequestContext, v: &Value) -> Result<Value> {
    let a = host.core();
    merchant(h)?;
    permit(h, "apps.manage")?;
    // Binary chunks have the existing asset byte budget, separate from the unchanged 64 KiB native metadata budget.
    let mut metadata = v.clone();
    metadata
        .as_object_mut()
        .ok_or(bad("Import object required"))?
        .remove("assetChunks");
    validate_native(&schema("merchant.import.record").unwrap(), &metadata)?;
    if v.get("assetChunks").is_some() && v["record"]["kind"] != "assets" {
        return Err(bad("Binary import requires an asset record"));
    }
    if v["approve"] != true {
        return Err(bad("Review and explicitly approve the source record"));
    }
    let record: SourceRecord =
        serde_json::from_value(v["record"].clone()).map_err(|_| bad("Invalid source record"))?;
    record.validate()?;
    if !record.warnings.is_empty() && v["options"]["acknowledgedWarnings"] != json!(record.warnings)
    {
        return Err(bad("Review all source limitations before importing"));
    }
    match record.kind.as_str() {
        "reviews" => reviews::import(host, h, &record).await,
        "coupons" => coupons::import(host, h, &record).await,
        "assets" => assets::import(host, h, &record, &v["assetChunks"]).await,
        "pages" => {
            host.import_record(
                h,
                &record.identity(&identity_namespace(a, &merchant(h)?).await?),
                &record.data["destination"],
                v["options"]["destinationDigest"]
                    .as_str()
                    .ok_or(bad("Reviewed destination package digest required"))?,
            )
            .await
        }
        "categories" => categories::import(host, h, &record, &v["options"]).await,
        "products" => products::import(a, h, &record, &v["options"]).await,
        "customers" => customers::import(a, h, &record, &v["options"]).await,
        "orders" => historical_orders::import(a, h, &record, &v["options"]).await,
        _ => Err(bad(
            "Unsupported native import domain; no target data changed",
        )),
    }
}
