//! The claim compiler renders only exact, current, confirmed and public claims; free prose is never certified.
use crate::store::evidence::{claim_rows, claims};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_kernel::verified_kernel;
use veyndo_marketing::service::channels::admit_product;

/// The JSON fields `compile` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CompileInput {
    claims: Value,
}

/// Renders requested statements that exactly match a current confirmed public claim, or refuses all.
pub async fn compile(a: &State, t: &str, product: &str, v: &Value) -> Result<Value> {
    let input: CompileInput = view(v);
    let facts = claim_rows(a, t, product, true).await?;
    let statements = input
        .claims
        .as_array()
        .filter(|a| a.len() <= 20)
        .ok_or(bad("At most 20 fact statements required"))?;
    let facts = facts.iter().map(view::<ClaimRecord>).collect::<Vec<_>>();
    let mut rendered = Vec::new();
    for s in statements {
        let s = view::<Statement>(s);
        let matched = facts.iter().find(|f| f.id == s.id && f.data.text == s.text);
        if let Some(f) = matched.filter(|f| {
            verified_kernel::claim_render_admissible(
                f.state == "confirmed",
                f.source_public == true,
                f.source_current == true,
                f.valid_time == true,
                f.data.text == s.text,
            )
        }) {
            rendered.push(json!({"id":f.id,"text":f.data.text,"locale":f.data.locale,"sourceId":f.data.source_id,"contentHash":f.data.content_hash}));
        } else {
            return Err(conflict(
                "Statement is not an exact current confirmed public claim",
            ));
        }
    }
    Ok(
        json!({"productId":product,"statements":rendered,"compiled":true,"scope":"Only these exact statements are checked; free prose is not certified"}),
    )
}
/// The public claims of a product the storefront may show.
pub async fn public_facts(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    // Reuse the Store API admission/visibility consumer, including channel restrictions.
    admit_product(&a, &h, &id).await?;
    Ok(Json(claims(&a, &tenant(&h)?, &id, true).await?))
}
/// Compiles storefront statements against the product's public claims.
pub async fn public_compile(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    admit_product(&a, &h, &id).await?;
    Ok(Json(compile(&a, &tenant(&h)?, &id, &v).await?))
}
/// The input schema of an experiment operation.
pub fn experiments_schema(name: &str) -> Option<Value> {
    let props = match name {
        "knowledge.experiments" | "knowledge.experiment.suggest" => json!({}),
        "knowledge.experiment.create" => {
            json!({"design":{"type":"object"},"approve":{"type":"boolean"}})
        }
        "knowledge.experiment.report" => json!({"id":{"type":"string"}}),
        "knowledge.experiment.start"
        | "knowledge.experiment.stop"
        | "knowledge.experiment.finish" => {
            json!({"id":{"type":"string"},"revision":{"type":"integer"},"approve":{"type":"boolean"}})
        }
        _ => return None,
    };
    Some(json!({"type":"object","properties":props,"additionalProperties":false}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn experiment_operations_have_closed_schemas() {
        assert!(experiments_schema("knowledge.experiments").is_some());
        assert_eq!(
            experiments_schema("knowledge.experiment.start").unwrap()["additionalProperties"],
            false
        );
        assert!(experiments_schema("knowledge.experiment.other").is_none());
    }
}

/// A requested fact statement.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Statement {
    id: Value,
    text: Value,
}

/// A stored claim with its admission flags.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ClaimRecord {
    id: Value,
    state: Value,
    source_public: Value,
    source_current: Value,
    valid_time: Value,
    #[serde(deserialize_with = "lenient")]
    data: ClaimText,
}

/// The claim text and the source it quotes.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ClaimText {
    text: Value,
    locale: Value,
    source_id: Value,
    content_hash: Value,
}
