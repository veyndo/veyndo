//! The unit order receipt PDF renders in the server after the B2B component admits the unit order.
use super::*;
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
    let snapshot: Value = sqlx::query_scalar(
        "SELECT snapshot FROM order_receipts WHERE tenant=$1 AND order_id=$2 AND id=$3",
    )
    .bind(tenant(&h)?)
    .bind(id)
    .bind(receipt)
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Receipt not found".into()))?;
    Ok(crate::operations::receipt_response(&snapshot))
}
