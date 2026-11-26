//! Safe structured rich content, never executable HTML. Same schema for merchant API and frontend.
use crate::domain::rich_document;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::localized::valid_locale_key;
use veyndo_core::{Result, bad};
/// The fields of a rich content block (and the type of a document root).
#[derive(Deserialize, Default)]
#[serde(default)]
struct Block {
    #[serde(deserialize_with = "lenient")]
    r#type: Option<String>,
    doc: Value,
    #[serde(deserialize_with = "lenient")]
    text: Option<String>,
    #[serde(deserialize_with = "lenient")]
    url: Option<String>,
}

pub fn validate_rich(value: &Value) -> Result<()> {
    let Some(languages) = value.as_object() else {
        return Err(bad("Rich descriptions require a language map"));
    };
    if languages.len() > 100 {
        return Err(bad("Too many rich description languages"));
    }
    for (lang, blocks) in languages {
        if !valid_locale_key(lang) {
            return Err(bad("Unsupported rich description locale"));
        }
        let blocks = blocks
            .as_array()
            .filter(|v| v.len() <= 40)
            .ok_or(bad("Maximum 40 rich description blocks"))?;
        for b in blocks {
            let block: Block = view(b);
            let typ = block
                .r#type
                .as_deref()
                .ok_or(bad("Rich block type required"))?;
            if typ == "document" {
                if view::<Block>(&block.doc).r#type.as_deref() != Some("doc") {
                    return Err(bad("Root document required"));
                }
                rich_document::validate(&block.doc, 0, &mut 0)?;
                continue;
            }
            if !["paragraph", "heading", "list", "image", "video"].contains(&typ)
                || block.text.as_deref().is_some_and(|s| s.len() > 4000)
            {
                return Err(bad("Invalid rich block"));
            }
            if ["image", "video"].contains(&typ) {
                let url = block.url.as_deref().ok_or(bad("Media URL required"))?;
                if url.len() > 2000 {
                    return Err(bad("Media URL too long"));
                }
                let parsed = reqwest::Url::parse(url)
                    .map_err(|_| bad("Absolute HTTPS media URL required"))?;
                if parsed.scheme() != "https"
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                {
                    return Err(bad("HTTPS media required"));
                }
            } else if block.text.is_none() {
                return Err(bad("Rich block text required"));
            }
        }
    }
    if value.to_string().len() > 512000 {
        return Err(bad("Rich content too large"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::validate_rich;
    use serde_json::json;
    #[test]
    fn executable_content_rejected() {
        assert!(
            validate_rich(&json!({"en":[{"type":"image","url":"javascript:alert(1)"}]})).is_err()
        );
        assert!(validate_rich(&json!({"de":[{"type":"paragraph","text":"**Hallo**"}]})).is_ok());
    }
}
