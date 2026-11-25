//! What a merchant may ask of an image job, and the instruction the image model receives.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Mode {
    Generate,
    Optimize,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Generate => "generate",
            Mode::Optimize => "optimize",
        }
    }
}

pub struct Request {
    pub prompt: String,
    pub mode: Mode,
    /// The uploaded image an optimization starts from.
    pub source: Option<String>,
}

/// The fields of an image job request, as sent.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RequestInput {
    #[serde(deserialize_with = "lenient")]
    prompt: Option<String>,
    #[serde(deserialize_with = "lenient")]
    mode: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_id: Option<String>,
}

pub fn parse(v: &Value) -> Result<Request> {
    let input: RequestInput = view(v);
    let prompt = input
        .prompt
        .filter(|s| !s.trim().is_empty() && s.len() <= 2000)
        .ok_or(bad("Image instructions required (up to 2000 bytes)"))?;
    let mode = match input.mode.as_deref() {
        Some("generate") => Mode::Generate,
        Some("optimize") => Mode::Optimize,
        _ => return Err(bad("Invalid image mode")),
    };
    Ok(Request {
        prompt,
        mode,
        source: input.source_id,
    })
}

/// The merchant's wish framed by the product facts, so the model keeps to the actual product.
pub fn instruction(name: &str, description: &str, prompt: &str) -> String {
    format!(
        "Product: {name}. Description: {description}. Merchant instructions: {prompt}. Preserve factual product shape, materials and proportions. Do not invent logos or product claims."
    )
}

#[cfg(test)]
mod tests {
    use super::{Mode, instruction, parse};
    use serde_json::json;

    #[test]
    fn a_request_needs_bounded_instructions_and_a_known_mode() {
        let ok = json!({"prompt":"White background","mode":"optimize","sourceId":"a1"});
        let request = parse(&ok).unwrap();
        assert_eq!(request.mode, Mode::Optimize);
        assert_eq!(request.source.as_deref(), Some("a1"));
        for value in [
            json!({"prompt":" ","mode":"generate"}),
            json!({"prompt":"x".repeat(2001),"mode":"generate"}),
            json!({"prompt":"ok","mode":"publish"}),
            json!({"mode":"generate"}),
        ] {
            assert!(parse(&value).is_err());
        }
    }

    #[test]
    fn the_instruction_carries_the_product_facts() {
        let text = instruction("Oak desk", "Solid oak", "Studio light");
        assert!(text.starts_with(
            "Product: Oak desk. Description: Solid oak. Merchant instructions: Studio light."
        ));
        assert!(text.contains("Do not invent logos"));
    }
    #[test]
    fn modes_have_their_wire_names() {
        assert_eq!(Mode::Generate.name(), "generate");
        assert_eq!(Mode::Optimize.name(), "optimize");
        for mode in [Mode::Generate, Mode::Optimize] {
            let value = json!({"prompt":"ok","mode":mode.name()});
            assert_eq!(parse(&value).unwrap().mode, mode);
        }
    }
}
