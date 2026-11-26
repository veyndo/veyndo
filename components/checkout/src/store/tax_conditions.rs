//! Tax rules with conditions read private pre-tax cart facts; the quote they are measured on carries no taxes of its own conditions.
use crate::domain::cart_quote::quote;
use serde_json::json;
use veyndo_core::Result;
use veyndo_marketing::service::conditions::{condition_context, match_condition};
use veyndo_pricing::catalog::Product;
use veyndo_shop::Settings;
use veyndo_shop::types::cart::StoredCart;

/// Whether any tax rule of the settings has a condition.
pub fn conditional(s: &Settings) -> bool {
    s.taxes
        .iter()
        .any(|t| t.rules.iter().any(|r| r.condition.is_some()))
}

/// The settings with every tax rule whose condition fails removed, and the remaining conditions settled.
pub async fn tax_settings_for_cart(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    ps: &[Product],
    s: &Settings,
) -> Result<Settings> {
    let mut resolved = s.clone();
    if !conditional(s) {
        return Ok(resolved);
    }
    let q = quote(c, ps, s)?;
    let definitions = json!(s.taxes);
    let context = condition_context(conn, c, &q, &definitions).await?;
    for t in &mut resolved.taxes {
        let mut accepted = Vec::new();
        for mut r in t.rules.drain(..) {
            let admitted = match &r.condition {
                Some(v) => match_condition(v, c, &context)?,
                None => true,
            };
            if admitted {
                r.condition = None;
                accepted.push(r);
            }
        }
        t.rules = accepted;
    }
    Ok(resolved)
}
