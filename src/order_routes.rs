//! Merchant order read route; the orders component reads and projects the orders.
use crate::*;

pub(crate) async fn orders(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "orders.read")?;
    auth::permit(&h, "customers.pii")?;
    Ok(Json(
        json!({"data":veyndo_orders::store::order_reads::recent(&a, &t).await?}),
    ))
}
