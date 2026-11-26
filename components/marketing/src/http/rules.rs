//! Rule comparison on frozen flow inputs and the revisioned rule facts of products, customers and orders.
use crate::domain::metadata::validate;
use crate::store::{rule_entities, rule_history};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::domain::permissions::allowed;
use veyndo_identity::{merchant, permit};

/// The rights a comparison needs, because frozen inputs carry order, customer and payment facts.
pub const COMPARISON_SCOPES: &[&str] = &[
    "automation.read",
    "orders.read",
    "catalog.read",
    "customers.read",
    "customers.pii",
    "payments.read",
    "documents.read",
    "b2b.read",
];

/// Whether a request holds every right of a rule comparison.
pub fn comparison_visible(h: &RequestContext) -> bool {
    COMPARISON_SCOPES.iter().all(|scope| allowed(h, scope))
}

/// The right a rule fact edit of an entity kind needs.
pub fn fact_scope(entity: &str) -> Result<&'static str> {
    match entity {
        "products" => Ok("catalog.write"),
        "customers" => Ok("customers.write"),
        "orders" => Ok("orders.write"),
        _ => Err(bad("Unknown rule entity")),
    }
}

/// Compares a saved rule and a draft on frozen order inputs, without replaying actions.
pub async fn compare(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    for scope in COMPARISON_SCOPES {
        permit(&h, scope)?;
    }
    Ok(Json(rule_history::compare(&a, &t, v).await?))
}

/// The JSON fields `save_facts` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveFactsInput {
    data: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Saves the rule facts of one product, customer or order under its revision.
pub async fn save_facts(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((entity, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SaveFactsInput = view(&v);
    let t = merchant(&h)?;
    permit(&h, fact_scope(&entity)?)?;
    validate(&entity, &input.data)?;
    let rev = input.revision.ok_or(bad("Revision required"))?;
    rule_entities::save(&a, &t, &entity, &id, &input.data, rev).await?;
    Ok(Json(json!({"saved":true,"revision":rev+1})))
}

#[cfg(test)]
mod tests {
    use super::{comparison_visible, fact_scope};
    use veyndo_core::RequestContext;

    #[test]
    fn comparisons_need_every_fact_right_and_facts_their_entity_right() {
        let mut admin = RequestContext::new();
        admin.insert("x-rac-role", "admin".parse().unwrap());
        assert!(comparison_visible(&admin));
        let mut viewer = RequestContext::new();
        viewer.insert("x-rac-role", "viewer".parse().unwrap());
        assert!(!comparison_visible(&viewer));
        assert_eq!(fact_scope("orders").unwrap(), "orders.write");
        assert_eq!(fact_scope("products").unwrap(), "catalog.write");
        assert!(fact_scope("payments").is_err());
    }
}
