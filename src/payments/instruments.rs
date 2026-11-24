//! Customer-owned saved instruments expose only redacted metadata; immediate revocation and provider cleanup share payment jobs.
use crate::*;
pub(crate) async fn identity(a: &App, h: &RequestContext) -> Result<(String, String)> {
    let (t, email) = accounts::identity(a, h).await?;
    let id = veyndo_customers::store::customers::active_id(a, &t, &email).await?;
    Ok((t, id))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    if name == "checkout.payment.methods" {
        list(a, h).await
    } else {
        if v["approve"] != true {
            return Err(bad("Approve saved payment revocation"));
        }
        revoke(
            a,
            h,
            v["id"].as_str().ok_or(bad("id required"))?,
            v["requestKey"].as_str().ok_or(bad("requestKey required"))?,
        )
        .await
    }
}
pub(crate) async fn list(a: &App, h: &RequestContext) -> Result<Value> {
    let (t, customer) = identity(a, h).await?;
    veyndo_payments::store::saved_instruments::list(a, &t, &customer).await
}
pub(crate) async fn revoke(a: &App, h: &RequestContext, id: &str, key: &str) -> Result<Value> {
    let (t, customer) = identity(a, h).await?;
    veyndo_payments::store::saved_instruments::revoke(a, &t, &customer, id, key).await
}
pub(crate) async fn route_list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(list(&a, &h).await?))
}
pub(crate) async fn route_revoke(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        revoke(
            &a,
            &h,
            &id,
            v["requestKey"]
                .as_str()
                .or_else(|| header(&h, "idempotency-key"))
                .ok_or(bad("requestKey required"))?,
        )
        .await?,
    ))
}
