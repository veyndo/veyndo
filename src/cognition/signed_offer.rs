//! Signed cart offers: an Ed25519 attestation of the native quote that a buying agent can check offline; a statement, not a price lock.
use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use veyndo_intelligence::domain::offer::payload;
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
