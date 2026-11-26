//! Typed proposals, list views and reviewed work steps, with their checks before persistence or execution.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_platform::domain::planning::AppChange;
use veyndo_catalog::domain::selection::validate as validate_selection;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad, conflict};
pub use veyndo_intelligence::domain::price_guardrails::Change;
use veyndo_intelligence::service::navigation::{Entity, Navigation};
use veyndo_pricing::catalog::Product;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A reviewed list view: a native entity with filter criteria and components.
pub struct View {
    /// `products`, `orders` or `customers`.
    pub entity: String,
    /// The native filter criteria.
    pub criteria: Value,
    /// The view components.
    #[serde(default = "default_components")]
    pub components: Vec<String>,
}
impl Entity for View {
    fn entity(&self) -> &str {
        &self.entity
    }
}
/// The default view components.
fn default_components() -> Vec<String> {
    vec!["records".into()]
}
/// The schema of a list view in a model response; the shipping policy needs knowledge rights.
pub fn view_schema(knowledge: bool) -> Value {
    let mut components = vec!["records", "details", "intake"];
    if knowledge {
        components.push("shippingPolicy");
    }
    // One explicit discriminator avoids provider preference for the first same-shaped union arm.
    // The exact per-entity filter schema is still enforced by admit(), after wire restoration.
    json!({"anyOf":[{"type":"object","properties":{
        "entity":{"type":"string","enum":["products","orders","customers"]},
        "criteria":{"type":"object"},
        "components":{"type":"array","minItems":1,"maxItems":3,"items":{"type":"string","enum":components},
          "description":"For delivery questions on orders, include details and shippingPolicy if available. This shows the native order details and current policy evidence."}
    },"required":["entity","criteria","components"],"additionalProperties":false},{"type":"null"}]})
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// One reviewed work step: a native operation or a managed app save with its arguments.
pub struct Step {
    /// The operation name.
    pub name: String,
    /// What the step does, for review.
    pub label: String,
    /// The JSON arguments.
    pub arguments_json: String,
    /// The bound app change of an app step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<AppChange>,
}
/// The native operations reviewed work may run.
pub const NATIVE: &[&str] = &[
    "merchant.product.create",
    "merchant.product.save",
    "merchant.category.create",
    "merchant.category.save",
    "automation.save",
    "merchant.translations.create",
];
/// The schema of a work plan in a model response.
pub fn work_schema() -> Value {
    json!({"type":"array","maxItems":12,"items":{"type":"object","required":["name","label","arguments_json"],"additionalProperties":false,"properties":{"name":{"type":"string","maxLength":160},"label":{"type":"string","minLength":1,"maxLength":200},"arguments_json":{"type":"string","maxLength":8192}}}})
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// A model proposal after native review.
pub struct Proposal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A Studio place to open.
    pub navigation: Option<Navigation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A list view to show.
    pub view: Option<View>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Reviewed native work.
    pub operations: Vec<Step>,
    #[serde(default)]
    /// One managed app save.
    pub app_action: Option<AppChange>,
    /// Queues catalogue drafts only after explicit approval; it never applies or publishes translations.
    #[serde(default)]
    pub translation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The explicitly selected products to translate.
    pub translation_products: Option<Vec<String>>,
    /// What the proposal does.
    pub summary: String,
    /// Price and stock changes.
    pub changes: Vec<Change>,
    #[serde(default)]
    /// A storefront experience change.
    pub experience: Option<Value>,
    #[serde(default)]
    /// The experience revision the change was reviewed against.
    pub expected_experience_revision: Option<i64>,
}
/// The JSON fields `validate_experience` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateExperienceInput {
    #[serde(deserialize_with = "lenient")]
    mode: Option<String>,
    #[serde(deserialize_with = "lenient")]
    headline: Option<String>,
}

/// Checks an experience change: a known mode and a bounded headline.
pub fn validate_experience(v: &Value) -> Result<()> {
    let input: ValidateExperienceInput = view(v);
    if !v.is_object() {
        return Err(bad("Experience must be an object"));
    }
    if !["balanced", "discovery", "comparison"].contains(&input.mode.as_deref().unwrap_or("")) {
        return Err(bad("Invalid experience mode"));
    }
    if input.headline.as_deref().is_none_or(|s| s.len() > 200) {
        return Err(bad("Experience headline must be <= 200 characters"));
    }
    Ok(())
}
/// Checks a proposal against the products it touches: bounds, uniqueness, revisions and ranges.
pub fn validate_proposal(p: &Proposal, ps: &[Product]) -> Result<()> {
    if p.changes.len() > 100 || p.summary.len() > 2000 {
        return Err(bad("Proposal is too large"));
    }
    if let Some(ids) = &p.translation_products {
        validate_selection(ids)?;
        if p.translation.is_none() {
            return Err(bad("Selected translations need a target language"));
        }
    }
    let mut ids = std::collections::HashSet::new();
    for c in &p.changes {
        if !ids.insert(&c.product_id) {
            return Err(bad("Duplicate product change"));
        }
        let product = ps
            .iter()
            .find(|p| p.id == c.product_id)
            .ok_or(bad("Unknown product in plan"))?;
        if c.expected_revision != product.revision {
            return Err(conflict("Model used a stale product revision"));
        }
        if c.price
            .is_some_and(|p| !p.is_finite() || !(0.01..=1_000_000.).contains(&p))
            || c.stock.is_some_and(|s| !(0..=1_000_000).contains(&s))
        {
            return Err(bad("Change outside allowed range"));
        }
    }
    if let Some(e) = &p.experience {
        validate_experience(e)?;
        if p.expected_experience_revision.is_none() {
            return Err(bad("Experience revision required"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::view_components::validate as validate_components;

    #[test]
    fn composition_contains_only_registered_components_for_the_native_entity() {
        assert!(
            validate_components(
                "products",
                &["records".into(), "details".into(), "intake".into()]
            )
            .is_ok()
        );
        assert!(
            validate_components("orders", &["records".into(), "shippingPolicy".into()]).is_ok()
        );
        assert!(
            !view_schema(false)
                .to_string()
                .contains("\"shippingPolicy\"")
        );
        assert!(view_schema(true).to_string().contains("\"shippingPolicy\""));
        for (entity, components) in [
            ("products", vec!["records", "sql"]),
            ("orders", vec!["records", "intake"]),
            ("products", vec!["records", "shippingPolicy"]),
            ("products", vec!["records", "records"]),
            ("customers", vec!["details"]),
            ("foreign", vec!["records"]),
        ] {
            assert!(
                validate_components(
                    entity,
                    &components
                        .into_iter()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn legacy_views_get_a_native_directory_and_cannot_carry_authority_or_data() {
        let value = json!({"entity":"products","criteria":{"active":true}});
        let view: View = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(view.components, vec!["records"]);
        for key in ["tenant", "rows", "permissions", "script", "request"] {
            let mut forged = value.clone();
            forged[key] = json!("foreign");
            assert!(serde_json::from_value::<View>(forged).is_err());
        }
    }
}

#[cfg(test)]
mod proposal_tests {
    use super::*;
    fn product() -> Product {
        serde_json::from_value(json!({
            "extra":{},"id":"p","name":"P","category":"","description":"","price":10.0,
            "tax_rate":19.0,"stock":1,"revision":3,"list_price":null,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }
    #[test]
    fn proposals_are_bounded_and_revision_bound() {
        assert!(validate_experience(&json!({"mode":"discovery","headline":"Hi"})).is_ok());
        assert!(validate_experience(&json!({"mode":"loud","headline":"Hi"})).is_err());
        let mut p: Proposal = serde_json::from_value(json!({"summary":"s","changes":[{"product_id":"p","price":12.0,"expected_revision":3}]})).unwrap();
        assert!(validate_proposal(&p, &[product()]).is_ok());
        p.changes[0].expected_revision = 2;
        assert!(validate_proposal(&p, &[product()]).is_err());
        assert!(view_schema(false).is_object());
        assert_eq!(work_schema()["maxItems"], 12);
        assert!(NATIVE.contains(&"merchant.product.save"));
    }
}
