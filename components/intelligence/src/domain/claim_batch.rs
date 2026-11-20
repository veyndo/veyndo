//! Bounded claim batches: a compilation certifies only statements that are exact current confirmed public claims.
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
                let exact = current.iter().any(|f| {
                    f["productId"] == binding.product_id
                        && f["id"] == statement.id
                        && f["text"] == statement.text
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
