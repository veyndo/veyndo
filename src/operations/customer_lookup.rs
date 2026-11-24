//! Fast customer-only lookup for native identity bindings; the search is owned by the customers component.
use super::Criteria;
use crate::{App, Json, RequestContext, Result, State, Value, auth, merchant};
pub(super) async fn lookup(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(c): axum::extract::Query<Criteria>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "customers.read")?;
    auth::permit(&h, "customers.pii")?;
    let limit = veyndo_studio::http::orders::validate_criteria(&c)?;
    Ok(Json(
        veyndo_customers::service::directory::lookup(&a.core, &t, &c.after, &c.query, limit)
            .await?,
    ))
}
