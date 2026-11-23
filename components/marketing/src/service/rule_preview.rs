//! Rule preview: a condition evaluated against the merchant's own priced cart with its rule facts; it changes nothing.
use crate::domain::rules::Condition;
use crate::store::facts::rule_facts;
use crate::store::rule_snapshot::attach;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};
use veyndo_shop::ports::Carts;

/// Whether the condition matches the request's cart.
pub async fn preview(a: &State, carts: &dyn Carts, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "automation.read")?;
    let c: Condition =
        serde_json::from_value(v["condition"].clone()).map_err(|_| bad("Unsupported condition"))?;
    c.validate(0)?;
    let cart = carts.load(h).await?;
    if cart.tenant != t {
        return Err(bad("Wrong cart"));
    }
    let mut q = carts.view(&cart).await?;
    q["ruleFacts"] = rule_facts(&mut *a.db.begin().await?, &cart, &q).await?;
    attach(
        &mut *a.db.begin().await?,
        &t,
        &v["condition"],
        &mut q["ruleFacts"],
    )
    .await?;
    Ok(json!({"matched":c.checked_matches(&cart,&q)?,"sideEffects":false}))
}
