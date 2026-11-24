//! The unit order receipt PDF renders in the server after the B2B component admits the unit order.
use crate::{App, Path, RequestContext, Response, Result, State, json, tenant};
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
