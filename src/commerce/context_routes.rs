//! Method discovery and checkout context changes are owned by the checkout component; the server supplies its ports.
use crate::{App, Json, RequestContext, Result, State, Value};

pub(crate) async fn options(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_checkout::service::checkout_options::options(&a, &crate::cart_price::Quotes(&a), &h)
            .await?,
    ))
}
pub(crate) async fn select_checkout(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let edits = crate::cart_mutation::Edits::new(&a, None, None);
    Ok(Json(
        veyndo_checkout::store::cart_selection::select(&a, &edits, &h, &v).await?,
    ))
}
