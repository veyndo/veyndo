//! Explicit product safety/sector facts, with market-language fallback; no inference of certifications.
use crate::types::legal::{Config, SECTORS};
use crate::types::settings::Settings;
use serde::Deserialize;
use serde_json::Value;
use veyndo_catalog::domain::content::translated_string;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};
pub const FIELDS: &[&str] = &[
    "manufacturer",
    "manufacturerAddress",
    "manufacturerContact",
    "responsiblePerson",
    "responsibleAddress",
    "responsibleContact",
    "identifier",
    "warnings",
    "fibres",
    "ingredients",
    "allergens",
    "nutrition",
    "netQuantity",
    "foodOperator",
    "origin",
    "instructions",
    "energyLabelUrl",
    "energySheetUrl",
    "registration",
    "ageVerification",
    "compatibility",
];
pub fn validate_product(v: &Value, s: &Settings) -> Result<()> {
    if v.is_null() {
        return Ok(());
    }
    let object = v
        .as_object()
        .ok_or(bad("Product compliance object required"))?;
    for (key, value) in object {
        if key == "sector" {
            if value.as_str().is_none_or(|v| !SECTORS.contains(&v)) {
                return Err(bad("Unknown compliance sector"));
            }
            continue;
        }
        if key == "nonEuManufacturer" {
            if !value.is_boolean() {
                return Err(bad("Manufacturer origin flag required"));
            }
            continue;
        }
        if !FIELDS.contains(&key.as_str()) {
            return Err(bad("Unknown product compliance field"));
        }
        let texts = value
            .as_object()
            .ok_or(bad("Localized compliance information required"))?;
        if texts.len() > 100
            || texts.iter().any(|(l, v)| {
                !s.languages().allowed(l)
                    || !v.is_null()
                        && v.as_str()
                            .is_none_or(|t| t.len() > 6000 || t.contains('\0'))
            })
        {
            return Err(bad("Invalid product compliance text"));
        }
    }
    Ok(())
}
/// The JSON fields `product_gaps` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProductGapsInput {
    compliance: Value,
    #[serde(deserialize_with = "lenient")]
    digital: Option<bool>,
}

/// The classification facts of a product's compliance object.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ComplianceFacts {
    sector: Value,
    non_eu_manufacturer: Value,
}

pub fn product_gaps(extra: &Value, config: &Config, locale: &str, main: &str) -> Vec<String> {
    let input: ProductGapsInput = view(extra);
    let v = &input.compliance;
    let facts: ComplianceFacts = view(v);
    let sector = facts
        .sector
        .as_str()
        .unwrap_or(if input.digital == Some(true) {
            "digital"
        } else {
            "general"
        });
    let mut required = vec![];
    if sector != "digital" && sector != "food" {
        required.extend([
            "manufacturer",
            "manufacturerAddress",
            "manufacturerContact",
            "identifier",
        ]);
    }
    if facts.non_eu_manufacturer == true {
        required.extend([
            "responsiblePerson",
            "responsibleAddress",
            "responsibleContact",
        ]);
    }
    match sector {
        "textiles" => required.push("fibres"),
        "food" => required.extend([
            "ingredients",
            "allergens",
            "nutrition",
            "netQuantity",
            "foodOperator",
        ]),
        "cosmetics" => required.extend(["ingredients", "instructions"]),
        "electronics" => required.push("instructions"),
        "digital" => required.push("compatibility"),
        "ageRestricted" => required.push("ageVerification"),
        _ => {}
    }
    // Sector inclusion activates checks for existing catalogs without a classified product.
    if config.sectors.contains(&"textiles".into()) && facts.sector.is_null() {
        required.push("fibres");
    }
    required
        .into_iter()
        .filter(|key| translated_string(&v[*key], locale, main).trim().is_empty())
        .map(String::from)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::{product_gaps, validate_product};
    use crate::types::settings::Settings;

    use crate::types::legal::Config;

    use serde_json::json;
    #[test]
    fn outside_eu_requires_responsible_person_and_textile_composition() {
        let v = json!({"compliance":{"sector":"textiles","nonEuManufacturer":true,"fibres":{"en-GB":"100% cotton"}}});
        let gaps = product_gaps(&v, &Config::default(), "de-DE", "en-GB");
        assert!(gaps.contains(&"responsibleContact".into()));
        assert!(!gaps.contains(&"fibres".into()));
    }
    #[test]
    fn compliance_facts_are_known_fields_with_texts_in_shop_languages() {
        let s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        assert!(validate_product(&json!(null), &s).is_ok());
        assert!(validate_product(&json!({"sector":"textiles","nonEuManufacturer":false,"fibres":{"de-DE":"100 % Baumwolle","en-GB":null}}), &s).is_ok());
        for invalid in [
            json!("text"),
            json!({"sector":"unknown-sector"}),
            json!({"nonEuManufacturer":"yes"}),
            json!({"certificate":{"de-DE":"CE"}}),
            json!({"fibres":"cotton"}),
            json!({"fibres":{"xx-XX":"cotton"}}),
            json!({"fibres":{"de-DE":7}}),
            json!({"fibres":{"de-DE":"a\0b"}}),
            json!({"fibres":{"de-DE":"x".repeat(6001)}}),
        ] {
            assert!(validate_product(&invalid, &s).is_err(), "{invalid}");
        }
    }
}
