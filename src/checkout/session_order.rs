//! Bind an explicitly reviewed session purchase to the native transaction and its original order.
use super::execute;
use crate::*;
use axum::http::header::CACHE_CONTROL;
pub(crate) use veyndo_checkout::domain::session_review::{Recovery, Review, key, receipt};
use veyndo_checkout::store::session;

pub(crate) async fn confirm_session(
    State(a): State<App>,
    mut h: RequestContext,
    Json(review): Json<Review>,
) -> Result<impl IntoResponse> {
    super::buyer_identity::bind(&mut h)?;
    if !review.confirmed {
        return Err(bad("Explicit buyer confirmation is required"));
    }
    if review.paypal_sandbox
        && (!runtime_config::get().checkout_paypal_sandbox
            || !super::buyer_identity::admitted_shop(&tenant(&h)?))
    {
        return Err(conflict(
            "Use the simulated payment method for this checkout profile",
        ));
    }
    let key = key(&review.session_id)?;
    let order = execute(&a, &h, &key, Some(&review)).await?;
    Ok((
        [(CACHE_CONTROL, "no-store")],
        Json(receipt(
            &order,
            &review.session_id,
            &review.cart_id,
            &review.customer_id,
            runtime_config::get().strict,
        )?),
    ))
}

pub(crate) async fn recover_session(
    State(a): State<App>,
    mut h: RequestContext,
    Json(input): Json<Recovery>,
) -> Result<impl IntoResponse> {
    super::buyer_identity::bind(&mut h)?;
    let key = key(&input.session_id)?;
    let tenant = tenant(&h)?;
    let mut tx = a.db.begin().await?;
    if b2b::identity(&mut tx, &h).await? != input.customer_id
        || staging::parent_conn(&mut tx, &tenant).await?.is_some()
    {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Checkout receipt unavailable".into(),
        ));
    }
    let order = session::order(
        &mut tx,
        &tenant,
        &key,
        &input.cart_id,
        marketing::channel_id(&h),
    )
    .await?;
    Ok((
        [(CACHE_CONTROL, "no-store")],
        Json(receipt(
            &order,
            &input.session_id,
            &input.cart_id,
            &input.customer_id,
            runtime_config::get().strict,
        )?),
    ))
}
