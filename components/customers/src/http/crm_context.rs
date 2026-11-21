//! The choices a CRM writer needs to create a customer, without granting settings access.
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::config;

/// Customer groups, countries and the main language of the shop.
pub async fn context(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    permit(&h, "customers.pii")?;
    let (s, _) = config(&a, &t).await?;
    Ok(Json(
        json!({"elements":s.customer_groups,"countries":s.countries,"mainLocale":s.main_locale}),
    ))
}
