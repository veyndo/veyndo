//! Bounded claim batches: a compilation certifies only statements that are exact current confirmed public claims.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veyndo_core::document::view;
use veyndo_core::{Result, bad, conflict};
use veyndo_kernel::verified_kernel;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Intake {
    pub product_ids: Vec<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Statement {
    pub id: String,
    pub text: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub product_id: String,
    pub claims: Vec<Statement>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compile {
    pub products: Vec<Binding>,
}
impl Compile {
    /// Rejects a batch beyond the statement and size bounds.
    pub fn bounded(&self) -> Result<()> {
        let v = self;
        if v.products.iter().any(|p| {
            p.claims.len() > 20
                || p.claims
                    .iter()
                    .any(|s| s.text.is_empty() || s.text.len() > 1200 || s.id.len() > 128)
        }) {
            return Err(bad("Claim compilation batch exceeds bounds"));
        }
        Ok(())
    }
    /// The products the batch speaks about.
    pub fn ids(&self) -> Vec<String> {
        self.products.iter().map(|p| p.product_id.clone()).collect()
    }
    /// Every statement must equal a current claim of its product.
    pub fn exact(&self, current: &[Value]) -> Result<()> {
        let v = self;
        for binding in &v.products {
            for statement in &binding.claims {
                let exact = current.iter().map(view::<CurrentClaim>).any(|f| {
                    f.product_id == binding.product_id
                        && f.id == statement.id
                        && f.text == statement.text
                });
                if !verified_kernel::claim_render_admissible(true, true, true, true, exact) {
                    return Err(conflict(
                        "Statement is not an exact current confirmed public claim",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A current confirmed claim of a product.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CurrentClaim {
    pub product_id: Value,
    pub id: Value,
    pub text: Value,
}

#[cfg(test)]
mod tests {
    use super::Compile;
    use serde_json::{Value, json};

    fn batch(claims: Value) -> Compile {
        serde_json::from_value(
            json!({"products":[{"productId":"p1","claims":claims},{"productId":"p2","claims":[]}]}),
        )
        .unwrap()
    }

    #[test]
    fn a_batch_is_bounded_per_product_and_statement() {
        let ok = batch(json!([{"id":"c1","text":"Waterproof to 50 m"}]));
        assert!(ok.bounded().is_ok());
        assert_eq!(ok.ids(), ["p1", "p2"]);
        assert!(batch(json!([{"id":"c1","text":""}])).bounded().is_err());
        assert!(
            batch(json!([{"id":"c1","text":"x".repeat(1201)}]))
                .bounded()
                .is_err()
        );
        assert!(
            batch(json!([{"id":"x".repeat(129),"text":"ok"}]))
                .bounded()
                .is_err()
        );
        assert!(
            batch(json!(vec![json!({"id":"c","text":"ok"}); 21]))
                .bounded()
                .is_err()
        );
        assert!(
            batch(json!(vec![json!({"id":"c","text":"ok"}); 20]))
                .bounded()
                .is_ok()
        );
    }

    #[test]
    fn only_exact_current_claims_of_the_same_product_are_certified() {
        let current = [
            json!({"productId":"p1","id":"c1","text":"Waterproof to 50 m"}),
            json!({"productId":"p2","id":"c2","text":"Made of steel"}),
        ];
        assert!(
            batch(json!([{"id":"c1","text":"Waterproof to 50 m"}]))
                .exact(&current)
                .is_ok()
        );
        assert!(batch(json!([])).exact(&[]).is_ok());
        assert!(
            batch(json!([{"id":"c1","text":"Waterproof to 100 m"}]))
                .exact(&current)
                .is_err()
        );
        assert!(
            batch(json!([{"id":"c2","text":"Made of steel"}]))
                .exact(&current)
                .is_err()
        );
        assert!(
            batch(json!([{"id":"c9","text":"Waterproof to 50 m"}]))
                .exact(&current)
                .is_err()
        );
    }
}
