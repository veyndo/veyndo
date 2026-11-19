//! Reorder proposals: a signed offer from the supplier shop's public checkout on this installation, checked against the ceiling; approval hands over to that checkout and orders nothing.
use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use veyndo_inventory::domain::reorder;
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
async fn offer(a: &App, buyer: &str, link: &supply::Link) -> Result<Value> {
    if link.supplier_shop == buyer {
        return Err(bad("A shop cannot be its own supplier"));
    }
    let visitor = guest(&link.supplier_shop, None)?;
    let cart = Box::pin(capabilities::invoke(
        a,
        &visitor,
        "cart.create",
        &json!({"session":format!("supply-{}", uid())}),
    ))
    .await?;
    let token = cart["token"]
        .as_str()
        .ok_or(bad("Supplier cart unavailable"))?
        .to_owned();
    let holder = guest(&link.supplier_shop, Some(&token))?;
    let revision = load_cart(a, &holder).await?.revision;
    let quote = Box::pin(capabilities::invoke(a, &holder, "cart.replace", &json!({"revision":revision,"items":[{"id":link.supplier_product_id,"quantity":link.reorder_quantity}]}))).await?;
    let signed = cognition::signed_offer::issue(a, &holder).await?;
    let stock = quote["lineItems"][0]["stock"].as_i64().unwrap_or(0);
    Ok(
        json!({"cartToken":token,"payload":signed["payload"],"payloadBase64":signed["payloadBase64"],"signature":signed["signature"],"keyId":signed["keyId"],
        "supplierStockEnough":stock >= i64::from(link.reorder_quantity)}),
    )
}
pub(crate) async fn propose(a: &App, h: &RequestContext) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "catalog.read")?;
    auth::permit(h, "settings.read")?;
    let (settings, revision) = commerce::config(a, &t).await?;
    let currency = settings.currencies.pricing_currency.clone();
    let scale = currencies::scale(&currency).ok_or(bad("Currency missing"))?;
    let ceiling = settings.supply.max_order_minor;
    let (mut lines, mut sufficient) = (Vec::new(), 0);
    for link in settings.supply.links.iter().take(50) {
        let row = sqlx::query("SELECT name,stock FROM products WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(&link.product_id)
            .fetch_optional(&a.db)
            .await?;
        let Some(row) = row else {
            lines.push(json!({"productId":link.product_id,"supplierShop":link.supplier_shop,"error":"product_missing"}));
            continue;
        };
        let stock: i32 = row.get("stock");
        if stock > link.reorder_point {
            sufficient += 1;
            continue;
        }
        let mut line = json!({"productId":link.product_id,"name":row.get::<String,_>("name"),"stock":stock,"reorderPoint":link.reorder_point,
            "quantity":link.reorder_quantity,"supplierShop":link.supplier_shop,"supplierProductId":link.supplier_product_id});
        match offer(a, &t, link).await {
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
pub(crate) async fn approve(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "settings.write")?;
    auth::permit(h, "catalog.read")?;
    if v["approve"] != true {
        return Err(bad("Explicit approve=true required"));
    }
    let (Some(product), Some(encoded), Some(signature), Some(token)) = (
        v["productId"].as_str(),
        v["payloadBase64"].as_str(),
        v["signature"].as_str(),
        v["cartToken"].as_str(),
    ) else {
        return Err(bad(
            "productId, payloadBase64, signature and cartToken required",
        ));
    };
    let (settings, _) = commerce::config(a, &t).await?;
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
    let checked = cognition::signed_offer::verify(
        a,
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
    let handoff = checkout_handoff::issue(
        State(a.clone()),
        holder,
        Json(json!({"revision":signed["cartRevision"]})),
    )
    .await?
    .0;
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'supply.reorder.approved',$2)")
        .bind(&t)
        .bind(json!({"productId":product,"supplierShop":link.supplier_shop,"quantity":link.reorder_quantity,"totalMinor":total.to_string(),"currency":signed["currency"],"actor":header(h,"x-rac-user")}))
        .execute(&a.db).await?;
    Ok(
        json!({"approved":true,"ordered":false,"supplierShop":link.supplier_shop,"checkoutPath":handoff["checkoutPath"],"expiresInSeconds":handoff["expiresInSeconds"],"totalMinor":total.to_string()}),
    )
}
async fn propose_http(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(propose(&a, &h).await?))
}
async fn approve_http(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(approve(&a, &h, &v).await?))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/merchant/supply/proposal",
            &[("POST", "settings.read")],
            post(propose_http),
        )
        .secure_route(
            "/api/merchant/supply/approve",
            &[("POST", "settings.write")],
            post(approve_http),
        )
}
