//! Caption proposals use central inference and current shop languages; only the owning native editor can save them.
use crate::ports::PublicEgress;
use crate::service::caption_source;
use crate::service::models::Models;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::admission::reserve_ai_attempt;
use veyndo_core::database::lease::Lease;
use veyndo_core::document::{lenient, view};
use veyndo_core::model_contract::validate_native;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::config;
const PROMPT: &str = include_str!("../../../../crates/cognition/prompts/media-caption.v1.txt");
/// The input schema of the caption capability.
pub fn schema(name: &str) -> Option<Value> {
    (name == "merchant.media.caption").then(|| json!({"type":"object","additionalProperties":false,"required":["locale","context"],"properties":{
        "locale":{"type":"string","minLength":2,"maxLength":35},
        "context":{"type":"string","minLength":1,"maxLength":4000},
        "imageUrl":{"type":"string","maxLength":2000},
        "alt":{"type":"string","maxLength":400},
        "caption":{"type":"string","maxLength":600}
    }}))
}
fn output_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["caption","alt"],"properties":{
        "caption":{"type":"string","minLength":1,"maxLength":600},"alt":{"type":"string","maxLength":400}
    }})
}
/// A caption request: the language, the image and the existing texts and context.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CaptionRequest {
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    image_url: Option<String>,
    context: Value,
    alt: Value,
    caption: Value,
}
/// A caption and alt text proposal in an enabled shop language; nothing is saved.
pub async fn prepare(
    m: Models<'_>,
    egress: &dyn PublicEgress,
    h: &RequestContext,
    v: &Value,
) -> Result<Value> {
    let a = m.core;
    let tenant = merchant(h)?;
    permit(h, "assistant.use")?;
    validate_native(&schema("merchant.media.caption").unwrap(), v)?;
    let input: CaptionRequest = view(v);
    let locale = input
        .locale
        .as_deref()
        .ok_or(bad("Caption language required"))?;
    let (settings, _) = config(a, &tenant).await?;
    if !settings.languages().allowed(locale) {
        return Err(bad("Caption language is not enabled for this shop"));
    }
    reserve_ai_attempt(a, &tenant).await?;
    let _slot = m.slots.clone().try_acquire_owned().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Caption model busy; retry shortly".into(),
        )
    })?;
    let _lease = Lease::acquire(a, &tenant, "model", 2).await?;
    let image =
        caption_source::read(egress, a, h, input.image_url.as_deref().unwrap_or("")).await?;
    let basis = if image.is_some() { "image" } else { "context" };
    let evidence = json!({"locale":locale,"context":input.context,"existingAlt":input.alt,"existingCaption":input.caption,"imageAvailable":image.is_some()});
    let result = m
        .inference
        .structured_with_images(
            None,
            PROMPT,
            &evidence.to_string(),
            &output_schema(),
            &image.into_iter().collect::<Vec<_>>(),
        )
        .await
        .map_err(|_| {
            Error(
                StatusCode::BAD_GATEWAY,
                "Caption provider unavailable; your content is unchanged".into(),
            )
        })?;
    validate_output(&result.value)?;
    Ok(
        json!({"caption":result.value["caption"],"alt":result.value["alt"],"locale":locale,"basis":basis,"promptId":"media-caption@1","model":result.model}),
    )
}
fn validate_output(v: &Value) -> Result<()> {
    validate_native(&output_schema(), v)?;
    for (field, limit) in [("caption", 600), ("alt", 400)] {
        let s = v[field].as_str().ok_or(bad("Invalid caption proposal"))?;
        if s.len() > limit || s.contains(['<', '>']) || s.chars().any(|c| c.is_control()) {
            return Err(bad("Caption proposal must be bounded plain text"));
        }
    }
    if v["caption"].as_str().unwrap().trim().is_empty() {
        return Err(bad("Caption proposal is empty"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::validate_output;
    use serde_json::json;
    #[test]
    fn proposals_are_text_not_actions_or_markup() {
        assert!(validate_output(&json!({"caption":"Ein Stuhl aus Holz.","alt":"Stuhl"})).is_ok());
        for value in [
            json!({"caption":"<img onerror=alert(1)>","alt":""}),
            json!({"caption":" ","alt":""}),
            json!({"caption":"text","alt":"","publish":true}),
            json!({"caption":"x".repeat(601),"alt":""}),
        ] {
            assert!(validate_output(&value).is_err());
        }
    }
}
