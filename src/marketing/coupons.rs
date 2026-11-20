//! Coupon selection is owned by the marketing component; the server supplies the guarded, priced cart.
use super::*;
pub(crate) async fn coupons(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_marketing::service::coupons::select(&crate::cart_storage::Carts(&a), &a, &h, &v)
            .await?,
    ))
}
