//! Merchant catalogue intake prepares bounded drafts; approved saves reuse native products, history and outbox in a private stage.
use crate::ports::{IntakeHost, PublicEgress};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_catalog::domain::intake_schema::schema;
use veyndo_core::model_contract::validate_native;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};
use veyndo_identity::{allowed, merchant};
use veyndo_pricing::types::currency::scale;
use veyndo_shop::Settings;
use veyndo_shop::service::read_cache::config;
mod csv;
#[path = "../../../../extensions/apps/catalog-import/mod.rs"]
mod inbound;
mod prepare;
mod save;

/// Context needs the catalogue read right, preparing and saving also the write right.
pub fn visible(h: &RequestContext, name: &str) -> bool {
    allowed(h, "catalog.read")
        && (name == "merchant.catalog.context" || allowed(h, "catalog.write"))
}
/// One catalogue intake operation: the shop context, draft preparation or the approved sandbox save.
pub async fn invoke(
    host: &dyn IntakeHost,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let a = host.models().core;
    let t = merchant(h)?;
    if !visible(h, name) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Catalogue intake requires current catalogue permissions".into(),
        ));
    }
    validate_native(
        &schema(name).ok_or(bad("Unknown catalogue intake operation"))?,
        v,
    )?;
    let (settings, settings_revision) = config(a, &t).await?;
    match name {
        "merchant.catalog.context" => Ok(
            json!({"mainLocale":settings.main_locale,"locales":settings.locales,"currency":settings.currencies.base_currency,"taxes":settings.taxes,"sandbox":veyndo_core::shop_scope::parent(a,&t).await?.is_some(),"settingsRevision":settings_revision,"agentic":host.readiness(h).await?}),
        ),
        "merchant.catalog.prepare" => {
            prepare::prepare(host, h, (&t, &settings, settings_revision), v).await
        }
        "merchant.catalog.save" => save::save(host, h, (&t, &settings), v).await,
        _ => Err(bad("Unknown catalogue intake operation")),
    }
}
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// One reviewed product draft; nothing is saved until an explicit approval.
pub(crate) struct Draft {
    pub name: String,
    pub description: String,
    pub price: f64,
    #[serde(default)]
    pub product_number: String,
    #[serde(default)]
    pub image_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<inbound::Source>,
}
impl Draft {
    /// Bounded name, price, description and a credential-free HTTPS image URL.
    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(source) = &self.source {
            source.validate()?;
        }
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || self.description.len() > 4000
            || self.product_number.len() > 100
            || !self.price.is_finite()
            || !(0.0..=1_000_000.).contains(&self.price)
            || self.image_url.len() > 2000
        {
            return Err(bad(
                "Product draft needs a name, valid price and bounded description",
            ));
        }
        if !self.image_url.is_empty() {
            let url = reqwest::Url::parse(&self.image_url)
                .map_err(|_| bad("Invalid product image URL"))?;
            if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
                return Err(bad(
                    "Product images require a public HTTPS URL without credentials",
                ));
            }
            // DNS pinning is checked before returning remotely loaded image references too.
        }
        Ok(())
    }
}

/// Reuse DNS-pinned public egress while allowing product query parameters, never URL credentials or redirects.
pub(super) async fn public_client(
    egress: &dyn PublicEgress,
    url: &reqwest::Url,
) -> Result<reqwest::Client> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(bad("Product URL credentials are forbidden"));
    }
    let mut origin = url.clone();
    origin.set_query(None);
    origin.set_fragment(None);
    egress.client(&origin).await
}
