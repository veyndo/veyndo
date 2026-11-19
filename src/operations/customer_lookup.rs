//! Fast customer-only lookup for native identity bindings; the search is owned by the customers component.
use super::*;
pub(super) async fn lookup(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(c): axum::extract::Query<Criteria>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "customers.read")?;
    auth::permit(&h, "customers.pii")?;
    let limit = super::list_criteria::validate(&c)?;
    Ok(Json(
        veyndo_customers::service::directory::lookup(&a.core, &t, &c.after, &c.query, limit)
            .await?,
    ))
}
