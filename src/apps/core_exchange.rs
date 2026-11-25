//! Generic channel/product projections and imported sales delegate to native owners with explicit app capabilities.
use super::{Manifest, approval, credentials, input_schema};
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{commerce, currencies};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::request_context::RequestContext;
/// The record, channel and currency a core exchange names.
/// The stored channel fields a marketplace exchange checks, compared exactly as stored.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MarketplaceChannel {
    kind: Value,
    active: Value,
    visibility: Value,
    product_ids: Value,
}

/// The product content and price a channel product read adds to the channel document.
#[derive(Serialize)]
struct ChannelProduct {
    product: Value,
    money: Value,
}

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
    let channel: MarketplaceChannel = view(&c);
    if channel.kind != "marketplace" || channel.active != true || channel.visibility == "private" {
        return Err(conflict("Marketplace channel must be active and public"));
    }
    let (cfg, rev) = commerce::scoped_config(a, &t, id).await?;
    let mut result = json!({"id":id,"revision":channel_revision,"data":c,"mainLocale":cfg.main_locale,"currencies":cfg.currencies,"settingsRevision":rev});
    if op == "channel_product" {
        let pid = input.id.as_deref().ok_or(bad("Product ID required"))?;
        let (p, visible) = veyndo_catalog::store::products::in_channel(a, &t, pid, id)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Product unavailable".into()))?;
        let selected = channel
            .product_ids
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
        let product =
            crate::operations::invoke(a, h, "merchant.product.content", &json!({"id":pid})).await?;
        let money = json!(currencies::amount(
            currencies::product_price(&p, &cfg.currencies, currency)?,
            &currency.code,
            currency.scale
        )?);
        merge(&mut result, ChannelProduct { product, money })?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::ChannelProduct;
    use serde_json::json;
    use veyndo_core::document::merge;

    #[test]
    fn channel_product_keeps_the_indexed_shape() {
        let result = json!({"id":"amazon","revision":3,"data":{"kind":"marketplace"},"mainLocale":"de-DE","currencies":{},"settingsRevision":9});
        let product = json!({"id":"p-1","name":{"de-DE":"Tasse"}});
        let money = json!({"amount":"12.50","currency":"EUR","minor":1250});
        let mut old = result.clone();
        old["product"] = product.clone();
        old["money"] = money.clone();
        let mut new = result;
        merge(&mut new, ChannelProduct { product, money }).unwrap();
        assert_eq!(new, old);
    }
}
