//! Versioned generic review records resolve native product identity and preserve source claims outside native purchase authority.
use super::SourceRecord;
use crate::ports::ImportHost;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{RequestContext, Result, bad, hash};
use veyndo_identity::merchant;
/// The source claims of a review record.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewSource {
    source_verified: Value,
    product_source_id: Value,
}
/// Imports one reviewed source review through the catalogue's review owner.
pub async fn import<H: ImportHost>(
    host: &H,
    h: &RequestContext,
    r: &SourceRecord,
) -> Result<Value> {
    let _a = host.core();
    let t = merchant(h)?;
    let claims: ReviewSource = view(&r.data);
    if !claims.source_verified.is_null() && !claims.source_verified.is_boolean() {
        return Err(bad("Source verification claim must be boolean or null"));
    }
    let product = super::relations::identity(
        r,
        &t,
        "products",
        claims
            .product_source_id
            .as_str()
            .ok_or(bad("Source product required"))?,
    )?;
    let source = json!({"version":r.version,"source":r.source,"sourceShop":r.source_shop,"sourceId":r.source_id,"recordDigest":hash(&json!(r).to_string()),"sourceVerified":claims.source_verified});
    veyndo_catalog::service::reviews::import(
        &host.catalog(),
        h,
        &r.identity(&t),
        &product,
        &r.data,
        &source,
    )
    .await
}
