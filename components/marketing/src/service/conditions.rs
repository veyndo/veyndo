//! Shared rule admission and authoritative context for tax and other native consumers.
use crate::domain::rules;
use crate::store::{facts, rule_snapshot};
use serde_json::Value;
use veyndo_core::{Result, bad};
use veyndo_shop::types::cart::StoredCart;
pub fn validate_condition(v: &Value) -> Result<()> {
    let c: rules::Condition =
        serde_json::from_value(v.clone()).map_err(|_| bad("Unsupported condition"))?;
    c.validate(0)
}
pub async fn condition_context(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    q: &Value,
    definitions: &Value,
) -> Result<Value> {
    let mut result = q.clone();
    result["ruleFacts"] = facts::rule_facts(conn, c, q).await?;
    rule_snapshot::attach(conn, &c.tenant, definitions, &mut result["ruleFacts"]).await?;
    Ok(result)
}
pub fn match_condition(v: &Value, c: &StoredCart, q: &Value) -> Result<bool> {
    let rule: rules::Condition =
        serde_json::from_value(v.clone()).map_err(|_| bad("Unsupported condition"))?;
    rule.validate(0)?;
    rule.checked_matches(c, q)
}
