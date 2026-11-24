//! The unit order receipt PDF renders in the server after the B2B component admits the unit order.
use crate::foundation::App;
use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
pub(super) async fn receipt(
    State(a): State<App>,
    h: RequestContext,
    Path((unit, id, receipt)): Path<(String, String, String)>,
) -> Result<Response> {
    veyndo_b2b::service::commands::unit_orders(
        &super::gateway::Procurement::new(&a),
        &a,
        &h,
        &json!({"unitId":unit,"id":id}),
    )
    .await?;
    let snapshot =
        veyndo_orders::store::receipts::stored_snapshot(&a, &tenant(&h)?, &id, &receipt).await?;
    Ok(crate::operations::receipt_response(&snapshot))
}
