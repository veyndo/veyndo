//! Signed cart offers: an Ed25519 attestation of the native quote that a buying agent can check offline; a statement, not a price lock.
use crate::domain::offer::payload;
use crate::service::fact_signing::{attest, verify as verify_signature};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, bad, conflict, shop_scope::tenant};
use veyndo_kernel::verified_kernel;
use veyndo_shop::ports::Carts;

/// Signs the current quote of the open cart.
pub async fn issue(carts: &dyn Carts, h: &RequestContext) -> Result<Value> {
    let cart = carts.load(h).await?;
    if cart.status != "open" {
        return Err(conflict("Only an open cart has an offer"));
    }
    let quote = carts.view(&cart).await?;
    attest(payload(
        &cart.tenant,
        &cart.data.sales_channel,
        &quote,
        chrono::Utc::now().timestamp(),
    )?)
}
/// Checks signature, shop, expiry and, when the caller holds the cart, whether checkout would still confirm it.
pub async fn verify(carts: &dyn Carts, h: &RequestContext, v: &Value) -> Result<Value> {
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
    if !verify_signature(&bytes, &signature)? {
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
    let current = match carts.load(h).await {
        Ok(cart) if cart.status == "open" => {
            let quote = carts.view(&cart).await?;
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
