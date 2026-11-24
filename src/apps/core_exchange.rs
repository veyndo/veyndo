//! Generic channel/product projections and imported sales delegate to native owners with explicit app capabilities.
use super::{Manifest, approval, credentials, input_schema};
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{commerce, currencies};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
/// The record, channel and currency a core exchange names.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ExchangeArgs {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    channel_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
}

pub(super) async fn execute(
    a: &App,
    h: &RequestContext,
    m: &Manifest,
    op: &str,
    v: &Value,
) -> Result<Value> {
    let input: ExchangeArgs = view(v);
    let t = merchant(a, h)?;
    let required = if op == "order_import" {
        vec![
            "orders.import",
            "payments.record",
            "orders.read",
            "customers.pii",
        ]
    } else {
        vec!["channels.read", "products.read"]
    };
    for p in required {
        if !m.permissions.iter().any(|q| q == p) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "App exchange permission not declared".into(),
            ));
        }
        credentials::permit(h, p)?;
    }
    if op == "order_import" {
        return commerce::import_external(a, h, &m.id, &approval::canonical_digest(m), v.clone())
            .await;
    }
    input_schema::validate(
        &json!({"type":"object","properties":{"id":{"type":"string","maxLength":100},"channelId":{"type":"string","maxLength":100},"currency":{"type":"string","maxLength":3}},"required":["channelId"],"additionalProperties":false}),
        v,
    )?;
    let id = input.channel_id.as_deref().unwrap();
    let (c, channel_revision) = veyndo_marketing::store::channel_config::stored(a, &t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Channel unavailable".into()))?;
    if c["kind"] != "marketplace" || c["active"] != true || c["visibility"] == "private" {
        return Err(conflict("Marketplace channel must be active and public"));
    }
    let (cfg, rev) = commerce::scoped_config(a, &t, id).await?;
    let mut result = json!({"id":id,"revision":channel_revision,"data":c,"mainLocale":cfg.main_locale,"currencies":cfg.currencies,"settingsRevision":rev});
    if op == "channel_product" {
        let pid = input.id.as_deref().ok_or(bad("Product ID required"))?;
        let (p, visible) = veyndo_catalog::store::products::in_channel(a, &t, pid, id)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Product unavailable".into()))?;
        let selected = c["productIds"]
            .as_array()
            .ok_or(bad("Invalid channel catalog"))?;
        if !visible
            || (!selected.is_empty()
                && !selected
                    .iter()
                    .any(|v| v == pid || p.parent_id.as_ref().is_some_and(|parent| v == parent)))
        {
            return Err(bad("Product unavailable in channel"));
        }
        let currency = cfg
            .currencies
            .selected(input.currency.as_deref().unwrap_or(""))?;
        result["product"] =
            crate::operations::invoke(a, h, "merchant.product.content", &json!({"id":pid})).await?;
        result["money"] = json!(currencies::amount(
            currencies::product_price(&p, &cfg.currencies, currency)?,
            &currency.code,
            currency.scale
        )?);
    }
    Ok(result)
}
