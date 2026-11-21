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

pub(super) use veyndo_customers::http::crm_context::context;
pub(super) use veyndo_customers::store::crm::address_plan;
