//! Generic coupon import delegates a reviewed native promotion definition, without pretending original usage is a new purchase.
use super::SourceRecord;
use crate::ports::ImportHost;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{RequestContext, Result, bad, hash};
use veyndo_identity::merchant;
use veyndo_pricing::types::currency::scale;
use veyndo_shop::service::read_cache::config;
/// The reviewed fields of a source coupon.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CouponSource {
    currency: Value,
    #[serde(deserialize_with = "lenient")]
    scale: Option<u64>,
    destination: Value,
}
/// Imports one reviewed source coupon as a native promotion definition.
pub async fn import<H: ImportHost>(
    host: &H,
    h: &RequestContext,
    r: &SourceRecord,
) -> Result<Value> {
    let a = host.core();
    let t = merchant(h)?;
    let settings = config(a, &t).await?.0;
    let source: CouponSource = view(&r.data);
    if source.currency != settings.currencies.pricing_currency
        || source.scale != scale(&settings.currencies.pricing_currency).map(u64::from)
    {
        return Err(bad(
            "Map source coupon amounts to the shop pricing currency before import",
        ));
    }
    let mut destination = source.destination;
    if !destination.is_object() {
        return Err(bad("Reviewed native coupon mapping required"));
    }
    merge(
        &mut destination,
        json!({ "sourceImport": {"version":r.version,"source":r.source,"sourceShop":r.source_shop,"sourceId":r.source_id,"currency":source.currency,"recordDigest":hash(&json!(r).to_string()),"activationReviewed":false} }),
    )?;
    host.import_promotion(h, &configuration_id(&r.identity(&t)), &destination)
        .await
}
pub fn configuration_id(id: &str) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let uuid = uuid::Uuid::parse_str(id).expect("native source UUID");
    let (mut value, mut bits) = (0_u32, 0_u32);
    let mut encoded = String::from("m_");
    for byte in uuid.as_bytes() {
        value = (value << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            encoded.push(ALPHABET[((value >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        encoded.push(ALPHABET[((value << (5 - bits)) & 31) as usize] as char);
    }
    encoded
}
