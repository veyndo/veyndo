//! Admin CRM creation uses native contacts/address storage, history and outbox; it never issues customer credentials.
use super::*;
pub(super) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    create_inner(a, h, v, None, None).await
}
pub(crate) async fn import(
    a: &App,
    h: &RequestContext,
    id: &str,
    v: Value,
    source: &Value,
) -> Result<Json<Value>> {
    create_inner(a.clone(), h.clone(), v, Some(id), Some(source)).await
}
async fn create_inner(
    a: App,
    h: RequestContext,
    v: Value,
    stable_id: Option<&str>,
    source: Option<&Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "customers.write")?;
    auth::permit(&h, "customers.pii")?;
    Ok(Json(
        veyndo_customers::store::crm::create(
            &a,
            &h,
            &t,
            v,
            stable_id,
            source,
            &marketing::validate_condition,
        )
        .await?,
    ))
}

/// Safe creation choices are available to CRM writers without granting settings access.
pub(super) async fn context(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "customers.write")?;
    auth::permit(&h, "customers.pii")?;
    let (s, _) = commerce::config(&a, &t).await?;
    Ok(Json(
        json!({"elements":s.customer_groups,"countries":s.countries,"mainLocale":s.main_locale}),
    ))
}

pub(super) use veyndo_customers::store::crm::address_plan;
