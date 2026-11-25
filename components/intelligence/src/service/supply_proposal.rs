//! Reorder proposals: a signed offer from the supplier shop's public checkout on this installation, checked against the ceiling; approval hands over to that checkout and orders nothing.
use crate::service::signed_offer;
use crate::store::product_reads::stock as product_stock;
use axum::Json;
use axum::extract::State as Shared;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_checkout::http::handoff;
use veyndo_checkout::store::cart_create::new_cart_context;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, bad, conflict, uid};
use veyndo_identity::{merchant, permit};
use veyndo_inventory::domain::{reorder, supply};
use veyndo_pricing::types::currency::scale;
use veyndo_shop::ports::Carts;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::cart::Item;

/// A guest at the supplier shop, exactly as any visitor: no merchant authority crosses the shop boundary.
fn guest(shop: &str, token: Option<&str>) -> Result<RequestContext> {
    let mut h = RequestContext::new();
    h.insert(
        "x-tenant",
        shop.parse().map_err(|_| bad("Invalid supplier shop"))?,
    );
    if let Some(token) = token {
        h.insert(
            "sw-context-token",
            token.parse().map_err(|_| bad("Invalid supplier cart"))?,
        );
    }
    Ok(h)
}

/// A fresh guest cart at the supplier holding the reorder quantity, and its signed offer.
async fn offer(a: &State, carts: &dyn Carts, buyer: &str, link: &supply::Link) -> Result<Value> {
    if link.supplier_shop == buyer {
        return Err(bad("A shop cannot be its own supplier"));
    }
    let visitor = guest(&link.supplier_shop, None)?;
    let cart = new_cart_context(a, &visitor, &format!("supply-{}", uid()), "mcp").await?;
    let holder = guest(&link.supplier_shop, Some(&cart.token))?;
    let revision = carts.load(&holder).await?.revision;
    let quantity =
        u32::try_from(link.reorder_quantity).map_err(|_| bad("Invalid reorder quantity"))?;
    let replaced = carts
        .replace(
            &holder,
            vec![Item {
                id: link.supplier_product_id.clone(),
                quantity,
            }],
            Some(revision),
        )
        .await?;
    let quote = carts.view(&replaced).await?;
    let signed = signed_offer::issue(carts, &holder).await?;
    let stock = quote["lineItems"][0]["stock"].as_i64().unwrap_or(0);
    Ok(
        json!({"cartToken":cart.token,"payload":signed["payload"],"payloadBase64":signed["payloadBase64"],"signature":signed["signature"],"keyId":signed["keyId"],
        "supplierStockEnough":stock >= i64::from(link.reorder_quantity)}),
    )
}

/// Every supplier link at or below its reorder point, with the supplier's signed offer and whether it fits the ceiling.
pub async fn propose(a: &State, carts: &dyn Carts, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "catalog.read")?;
    permit(h, "settings.read")?;
    let (settings, revision) = settings(a, &t, "default").await?;
    let currency = settings.currencies.pricing_currency.clone();
    let scale = scale(&currency).ok_or(bad("Currency missing"))?;
    let ceiling = settings.supply.max_order_minor;
    let (mut lines, mut sufficient) = (Vec::new(), 0);
    for link in settings.supply.links.iter().take(50) {
        let row = product_stock(&a.db, &t, &link.product_id).await?;
        let Some(row) = row else {
            lines.push(json!({"productId":link.product_id,"supplierShop":link.supplier_shop,"error":"product_missing"}));
            continue;
        };
        let stock = row.stock;
        if stock > link.reorder_point {
            sufficient += 1;
            continue;
        }
        let mut line = json!({"productId":link.product_id,"name":row.name,"stock":stock,"reorderPoint":link.reorder_point,
            "quantity":link.reorder_quantity,"supplierShop":link.supplier_shop,"supplierProductId":link.supplier_product_id});
        match offer(a, carts, &t, link).await {
            Ok(found) => {
                let total = found["payload"]["totalMinor"]
                    .as_str()
                    .and_then(|v| v.parse::<u64>().ok());
                let same = found["payload"]["currency"].as_str() == Some(currency.as_str());
                line["offer"] = found;
                line["sameCurrency"] = json!(same);
                line["withinCeiling"] = json!(reorder::within_ceiling(same, total, ceiling));
            }
            Err(e) => {
                line["error"] = json!("supplier_refused");
                line["detail"] = json!(e.1.chars().take(200).collect::<String>());
            }
        }
        lines.push(line);
    }
    Ok(
        json!({"lines":lines,"sufficient":sufficient,"links":settings.supply.links.len(),"maxOrderMinor":ceiling,"currency":currency,"scale":scale,
        "settingsRevision":revision,"network":"same-installation","orders":"none"}),
    )
}

/// Approval re-checks the signed offer and the ceiling, then issues the supplier's own single-use checkout link.
/// An approved signed supply proposal and the cart it was made for.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SignedSupply {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    payload_base64: Option<String>,
    #[serde(deserialize_with = "lenient")]
    signature: Option<String>,
    #[serde(deserialize_with = "lenient")]
    cart_token: Option<String>,
}

pub async fn approve(a: &State, carts: &dyn Carts, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "settings.write")?;
    permit(h, "catalog.read")?;
    let input: SignedSupply = view(v);
    if input.approve != Some(true) {
        return Err(bad("Explicit approve=true required"));
    }
    let (Some(product), Some(encoded), Some(signature), Some(token)) = (
        input.product_id.as_deref(),
        input.payload_base64.as_deref(),
        input.signature.as_deref(),
        input.cart_token.as_deref(),
    ) else {
        return Err(bad(
            "productId, payloadBase64, signature and cartToken required",
        ));
    };
    let (settings, _) = settings(a, &t, "default").await?;
    let link = settings
        .supply
        .links
        .iter()
        .find(|l| l.product_id == product)
        .ok_or(bad("No supplier link for this product"))?;
    let ceiling = settings
        .supply
        .max_order_minor
        .ok_or(conflict("Set a ceiling per reorder before approving one"))?;
    let holder = guest(&link.supplier_shop, Some(token))?;
    let checked = signed_offer::verify(
        carts,
        &holder,
        &json!({"payloadBase64":encoded,"signature":signature}),
    )
    .await?;
    if checked["honoured"] != true {
        return Err(conflict(
            "The supplier's offer is no longer valid; request a new proposal",
        ));
    }
    let signed: Value = URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or(bad("Invalid offer"))?;
    let total = reorder::admit(
        &signed,
        link,
        &settings.currencies.pricing_currency,
        ceiling,
    )?;
    let handoff = handoff::issue(
        Shared(a.clone()),
        holder,
        Json(json!({"revision":signed["cartRevision"]})),
    )
    .await?
    .0;
    veyndo_core::store::outbox::publish_on(&a.db, &t, "supply.reorder.approved", &json!({"productId":product,"supplierShop":link.supplier_shop,"quantity":link.reorder_quantity,"totalMinor":total.to_string(),"currency":signed["currency"],"actor":header(h,"x-rac-user")})).await?;
    Ok(
        json!({"approved":true,"ordered":false,"supplierShop":link.supplier_shop,"checkoutPath":handoff["checkoutPath"],"expiresInSeconds":handoff["expiresInSeconds"],"totalMinor":total.to_string()}),
    )
}
