//! Signed cart offers: an Ed25519 attestation of the native quote that a buying agent can check offline; a statement, not a price lock.
use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
const VALID_SECONDS: i64 = 600;
/// The positive projection that is signed: totals, lines and the reviewed selection, never buyer data or the cart capability.
fn payload(t: &str, channel: &str, q: &Value, now: i64) -> Result<Value> {
    let lines = q["lineItems"]
        .as_array()
        .filter(|lines| !lines.is_empty())
        .ok_or(conflict("An empty cart has no offer"))?;
    let total = q["price"]["totalMinor"]
        .as_str()
        .ok_or(bad("Native quote has no exact total"))?;
    Ok(
        json!({"schemaVersion":1,"kind":"offer","tenant":t,"channel":channel,
        "cartId":q["id"],"cartRevision":q["revision"],"configurationRevision":q["configurationRevision"],
        "currency":q["price"]["currency"],"currencyScale":q["price"]["currencyScale"],"totalMinor":total,"totalPrice":q["price"]["totalPrice"],
        "shippingPrice":q["shippingCosts"]["totalPrice"],
        "country":q["checkout"]["country"],"shippingMethodId":q["checkout"]["shippingMethodId"],"paymentMethodId":q["checkout"]["paymentMethodId"],
        "lineItems":lines.iter().map(|i|json!({"id":i["id"],"label":i["label"],"quantity":i["quantity"],"unitPrice":i["price"]["unitPrice"],"totalPrice":i["price"]["totalPrice"]})).collect::<Vec<_>>(),
        "issuedAt":now,"expiresAt":now+VALID_SECONDS,
        "meaning":"The shop's calculated quote for this cart revision at issue time. Checkout confirms exactly this revision and total, or refuses and a new offer is needed. It reserves no stock and does not lock a price the merchant changes afterwards."}),
    )
}
pub(crate) async fn issue(a: &App, h: &RequestContext) -> Result<Value> {
    let cart = load_cart(a, h).await?;
    if cart.status != "open" {
        return Err(conflict("Only an open cart has an offer"));
    }
    let quote = cart_json(a, &cart).await?;
    signed::attest(payload(
        &cart.tenant,
        &cart.data.sales_channel,
        &quote,
        chrono::Utc::now().timestamp(),
    )?)
}
/// Checks signature, shop, expiry and, when the caller holds the cart, whether checkout would still confirm it.
pub(crate) async fn verify(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = tenant(h)?;
    let (Some(encoded), Some(signature)) = (v["payloadBase64"].as_str(), v["signature"].as_str())
    else {
        return Err(bad("payloadBase64 and signature required"));
    };
    if encoded.len() > 65536 || signature.len() > 256 {
        return Err(bad("Offer too large"));
    }
    let refuse = |reason: &str| Ok(json!({"valid":false,"reason":reason}));
    let (Ok(bytes), Ok(signature)) = (
        URL_SAFE_NO_PAD.decode(encoded),
        URL_SAFE_NO_PAD.decode(signature),
    ) else {
        return refuse("malformed");
    };
    if !signed::verify(&bytes, &signature)? {
        return refuse("signature");
    }
    let Ok(offer) = serde_json::from_slice::<Value>(&bytes) else {
        return refuse("malformed");
    };
    if offer["kind"] != "offer" || offer["tenant"].as_str() != Some(t.as_str()) {
        return refuse("other_shop");
    }
    let expired = offer["expiresAt"]
        .as_i64()
        .is_none_or(|at| at < chrono::Utc::now().timestamp());
    // Without the cart capability only the signature and expiry can be judged.
    let current = match load_cart(a, h).await {
        Ok(cart) if cart.status == "open" => {
            let quote = cart_json(a, &cart).await?;
            Some((
                quote["id"] == offer["cartId"],
                quote["revision"] == offer["cartRevision"],
                quote["price"]["totalMinor"] == offer["totalMinor"],
            ))
        }
        Ok(_) => Some((false, false, false)),
        Err(_) => None,
    };
    let honoured = current.is_some_and(|(cart, revision, total)| {
        verified_kernel::offer_honoured(expired, cart, revision, total)
    });
    let current = current.map(|(cart, revision, total)| cart && revision && total);
    Ok(
        json!({"valid":true,"expired":expired,"matchesCart":current,"honoured":honoured,
        "totalMinor":offer["totalMinor"],"currency":offer["currency"],"cartRevision":offer["cartRevision"],"expiresAt":offer["expiresAt"]}),
    )
}
async fn issue_http(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(issue(&a, &h).await?))
}
async fn verify_http(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(verify(&a, &h, &v).await?))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/checkout/cart/offer", get(issue_http))
        .route("/store-api/checkout/offer/verify", post(verify_http))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signs_only_the_closed_projection_and_refuses_an_empty_cart() {
        let quote = json!({"id":"c1","revision":4,"configurationRevision":9,"token":"secret-capability","customerEmail":"buyer@example.test",
            "checkout":{"country":"DE","shippingMethodId":"standard","paymentMethodId":"demo-card","address":{"street":"Private 1"}},
            "price":{"currency":"EUR","currencyScale":2,"totalMinor":"12290","totalPrice":122.9},"shippingCosts":{"totalPrice":4.9},
            "paymentMethod":{"id":"demo-card","name":"Demo"},
            "lineItems":[{"id":"scarf","label":"Scarf","quantity":2,"stock":30,"price":{"unitPrice":59.0,"totalPrice":118.0}}]});
        let offer = payload("shop", "default", &quote, 1000).unwrap();
        let text = offer.to_string();
        assert!(
            !text.contains("secret-capability")
                && !text.contains("buyer@example.test")
                && !text.contains("Private 1")
        );
        assert!(offer["lineItems"][0].get("stock").is_none());
        assert_eq!(offer["totalMinor"], "12290");
        assert_eq!(offer["cartRevision"], 4);
        assert_eq!(offer["currency"], "EUR");
        assert_eq!(offer["channel"], "default");
        assert_eq!(offer["paymentMethodId"], "demo-card");
        assert_eq!(offer["expiresAt"], 1600);
        assert_eq!(
            offer["lineItems"][0],
            json!({"id":"scarf","label":"Scarf","quantity":2,"unitPrice":59.0,"totalPrice":118.0})
        );
        let mut empty = quote.clone();
        empty["lineItems"] = json!([]);
        assert!(payload("shop", "default", &empty, 1000).is_err());
    }
}
