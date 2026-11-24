//! Inbound provider formats belong to this app: product drafts, Shopify product exports and read-only Shop MCP searches; reviewed creation and publication remain native Veyndo operations.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_catalog::domain::csv_rows as csv;
use veyndo_core::{Result, bad, hash};
use veyndo_pricing::types::currency::scale;
use veyndo_shop::Settings;
mod shop_mcp;
mod shopify_csv;
pub use shop_mcp::{ENDPOINT, prepare as shop_mcp};
pub use shopify_csv::parse as shopify_csv;
/// Where an imported draft came from.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub provider: String,
    pub external_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}
/// Provider HTML is content only. Skip executable/style content; return plain bounded text, never rendered HTML.
fn plain(html: &str) -> String {
    let mut text = String::new();
    let mut tag = String::new();
    let mut inside = false;
    let mut hidden = false;
    for ch in html.chars() {
        if ch == '<' {
            inside = true;
            tag.clear();
        } else if ch == '>' && inside {
            inside = false;
            let name = tag.trim().to_ascii_lowercase();
            if name.starts_with("script") || name.starts_with("style") {
                hidden = true;
            }
            if name.starts_with("/script") || name.starts_with("/style") {
                hidden = false;
            }
            if !hidden {
                text.push(' ');
            }
        } else if inside {
            tag.push(ch);
        } else if !hidden {
            text.push(ch);
        }
    }
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Source {
    /// The provenance names a known provider, a bounded external ID and a credential-free HTTPS URL.
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.provider.as_str(), "shopify_csv" | "shop_mcp")
            || self.external_id.is_empty()
            || self.external_id.len() > 500
            || self.url.len() > 2000
        {
            return Err(bad("Invalid import provenance"));
        }
        if !self.url.is_empty() {
            let url = reqwest::Url::parse(&self.url).map_err(|_| bad("Invalid supplier URL"))?;
            if url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.host_str().is_none()
            {
                return Err(bad(
                    "Supplier provenance requires a credential-free HTTPS URL",
                ));
            }
        }
        Ok(())
    }
}

/// One reviewed product draft; nothing is saved until an explicit approval.
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub name: String,
    pub description: String,
    pub price: f64,
    #[serde(default)]
    pub product_number: String,
    #[serde(default)]
    pub image_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}
impl Draft {
    /// Bounded name, price, description and a credential-free HTTPS image URL.
    pub fn validate(&self) -> Result<()> {
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
