//! Standard-shop page contract. App-owned content and theme validation; no HTML, CSS or script evaluation.
use axum::Router;
use axum::extract::FromRef;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
mod block_validation;
mod context;
pub mod design_agent;
mod elements;
mod planning;
pub mod ports;
pub use ports::SiteHost;
/// What the site builder routes extract from the server state: the host port of this process.
#[derive(Clone)]
pub struct SiteState(pub Arc<dyn SiteHost>);

/// The page context and design routes; the server provides `SiteState` from its own state.
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    SiteState: FromRef<S>,
{
    Router::new()
        .merge(context::router())
        .merge(design_agent::router())
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Content {
    pub blocks: Vec<Block>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Block {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub text: HashMap<String, Option<String>>,
    #[serde(default)]
    pub rich: Value,
    #[serde(default)]
    pub image: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub caption: HashMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub image_alt: HashMap<String, Option<String>>,
    #[serde(default)]
    pub products: Vec<String>,
    #[serde(default)]
    pub product_mode: Option<String>,
    #[serde(default)]
    pub fields: Vec<FormField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<ElementRef>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ElementRef {
    pub app: String,
    pub surface: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FormField {
    pub id: String,
    pub kind: String,
    pub required: bool,
    pub label: HashMap<String, String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Theme {
    accent: Option<String>,
    background: Option<String>,
    foreground: Option<String>,
    font: Option<String>,
    radius: Option<u8>,
    spacing: Option<u8>,
    columns: Option<u8>,
}
pub fn normalize(v: &Value) -> Result<Value> {
    let mut next = v.clone();
    let content: Content =
        serde_json::from_value(next["content"].clone()).map_err(|_| bad("Invalid page blocks"))?;
    next["content"] = serde_json::to_value(content).map_err(|_| bad("Invalid page blocks"))?;
    Ok(next)
}
/// The JSON fields `validate` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateInput {
    content: Value,
    theme: Value,
    #[serde(deserialize_with = "lenient")]
    slug: Option<String>,
}

pub fn validate(entity: &str, v: &Value) -> Result<()> {
    let input: ValidateInput = view(v);
    if entity != "pages" {
        return Err(bad("Unknown site model"));
    }
    let content: Content =
        serde_json::from_value(input.content.clone()).map_err(|_| bad("Invalid page blocks"))?;
    let theme: Theme =
        serde_json::from_value(input.theme.clone()).map_err(|_| bad("Invalid shop theme"))?;
    let slug = input.slug.as_deref().unwrap_or("");
    if slug.is_empty()
        || slug.len() > 100
        || !slug
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || content.blocks.len() > 24
    {
        return Err(bad("Use a lowercase page slug and at most 24 blocks"));
    }
    block_validation::validate(&content.blocks)?;
    block_validation::validate_layout(slug, &content.blocks)?;
    for c in [theme.accent, theme.background, theme.foreground]
        .into_iter()
        .flatten()
    {
        if c.len() != 7
            || !c.starts_with('#')
            || !c.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(bad("Theme colors require #RRGGBB"));
        }
    }
    if theme
        .font
        .as_ref()
        .is_some_and(|f| !["sans", "serif", "mono"].contains(&f.as_str()))
        || theme.radius.is_some_and(|n| n > 32)
        || theme.spacing.is_some_and(|n| !(12..=96).contains(&n))
        || theme.columns.is_some_and(|n| !(1..=4).contains(&n))
    {
        return Err(bad("Theme values exceed design bounds"));
    }
    Ok(())
}
fn safe_image(s: &str) -> bool {
    (s.starts_with("/store-api/") && !s.contains(['\\', '\r', '\n']))
        || reqwest::Url::parse(s).is_ok_and(|u| {
            u.scheme() == "https" && u.username().is_empty() && u.password().is_none()
        })
}
/// The JSON fields `validate_owned` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateOwnedInput {
    #[serde(deserialize_with = "lenient")]
    channel: Option<String>,
    #[serde(deserialize_with = "lenient")]
    slug: Option<String>,
    content: Value,
}

pub async fn validate_owned(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    v: &Value,
    record_id: &str,
) -> Result<()> {
    let input: ValidateOwnedInput = view(v);
    let channel = input
        .channel
        .as_deref()
        .ok_or(bad("Page channel required"))?;
    if channel != "*"
        && !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2)",
        )
        .bind(t)
        .bind(channel)
        .fetch_one(&mut **tx)
        .await?
    {
        return Err(bad("Page channel does not belong to this shop"));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,947))")
        .bind(format!("{t}:site_routes"))
        .execute(&mut **tx)
        .await?;
    sqlx::query("SELECT set_config('rac.tenant',$1,true)")
        .bind(t)
        .execute(&mut **tx)
        .await?;
    if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM app_records WHERE tenant=$1 AND app='site_builder' AND entity='pages' AND fields->>'channel'=$2 AND fields->>'slug'=$3 AND id<>$4)")
        .bind(t)
        .bind(channel)
        .bind(input.slug.as_deref())
        .bind(record_id)
        .fetch_one(&mut **tx)
        .await?
    {
        return Err(conflict("A page already uses this address in this channel"));
    }
    let settings: Value = sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1")
        .bind(t)
        .fetch_one(&mut **tx)
        .await?;
    let settings = veyndo_shop::domain::defaults::decode(settings)?;
    let content: Content =
        serde_json::from_value(input.content.clone()).map_err(|_| bad("Invalid page"))?;
    for b in content.blocks {
        if let Some(element) = &b.extension {
            elements::validate_owned(tx, t, element).await?;
        }
        let mut languages = b
            .text
            .keys()
            .chain(b.caption.keys())
            .chain(b.image_alt.keys())
            .cloned()
            .collect::<Vec<_>>();
        if let Some(rich) = b.rich.as_object() {
            languages.extend(rich.keys().cloned());
        }
        for f in &b.fields {
            languages.extend(f.label.keys().cloned());
        }
        if languages.iter().any(|k| {
            !settings
                .locales
                .iter()
                .any(|l| l == k || l.split('-').next() == Some(k.as_str()))
        }) {
            return Err(bad(
                "Page translation language is not enabled for this shop",
            ));
        }

        for id in b.products {
            if !sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)",
            )
            .bind(t)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?
            {
                return Err(bad("Page product does not belong to this shop"));
            }
        }
    }
    Ok(())
}
pub use planning::contract as planning_contract;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn page_rejects_scripts_unbounded_theme_and_duplicate_ids() {
        let mut v = json!({"slug":"home","content":{"blocks":[]},"theme":{}});
        assert!(validate("pages", &v).is_ok());
        v["theme"]["accent"] = json!("url(javascript:alert(1))");
        assert!(validate("pages", &v).is_err());
        v["theme"] = json!({});
        v["content"]["blocks"] = json!([{"id":"x","kind":"image","image":"javascript:alert(1)"}]);
        assert!(validate("pages", &v).is_err());
    }
}
