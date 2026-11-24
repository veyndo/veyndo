//! Customer receipt PDFs render in the server; the order reads and projection are owned by the orders component.
use super::identity;
use crate::{App, Error, Path, RequestContext, Response, Result, State, StatusCode};
use veyndo_orders::http::account_orders::owned;
pub(crate) use veyndo_orders::http::account_orders::{detail, list, project};
use veyndo_orders::store::account_orders;
pub(super) async fn receipt(
    State(a): State<App>,
    h: RequestContext,
    Path((order, id)): Path<(String, String)>,
) -> Result<Response> {
    let (t, email) = identity(&a, &h).await?;
    if owned(&a, &t, &email, Some(&order), None).await?.is_empty() {
        return Err(Error(StatusCode::NOT_FOUND, "Order not found".into()));
    }
    let (snapshot, content) = account_orders::receipt(&a, &t, &order, &id).await?;
    Ok(crate::operations::document_response(&snapshot, content))
}
