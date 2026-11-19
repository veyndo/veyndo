//! Native currency discovery, revision-safe selection and authenticated FX/price job operations.
use super::*;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route(
            "/store-api/currencies",
            get(veyndo_pricing::http::currencies::discover),
        )
        .route("/store-api/checkout/currency", axum::routing::put(select))
        .secure_route(
            "/api/merchant/currencies/rates/refresh",
            &[("POST", "catalog.write")],
            post(refresh),
        )
        .secure_route(
            "/api/merchant/currencies/price-jobs",
            &[("GET", "catalog.read"), ("POST", "catalog.write")],
            get(veyndo_pricing::http::price_jobs::list)
                .post(veyndo_pricing::http::price_jobs::create),
        )
        .secure_route(
            "/api/merchant/currencies/price-jobs/{id}",
            &[("GET", "catalog.read")],
            get(veyndo_pricing::http::price_jobs::detail),
        )
}
pub(super) async fn select(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let current = load_cart(&a, &h).await?;
    let (s, _) = commerce::scoped_config(&a, &current.tenant, &current.data.sales_channel).await?;
    let code = s
        .currencies
        .selected(v["currency"].as_str().ok_or(bad("Currency required"))?)?
        .code
        .clone();
    let mut c = current.clone();
    c.data.currency = code;
    if c.status != "open" || v["revision"].as_i64() != Some(c.revision) {
        return Err(conflict("Cart changed or terminal"));
    }
    c.revision += 1;
    let q = cart_json(&a, &c).await?;
    let n=sqlx::query("UPDATE carts SET data=$1,revision=$2 WHERE tenant=$3 AND token=$4 AND revision=$5 AND status='open'").bind(json!(c.data)).bind(c.revision).bind(&c.tenant).bind(&c.token).bind(current.revision).execute(&a.db).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Cart changed; reload first"));
    }
    Ok(Json(q))
}
pub(super) async fn refresh(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    auth::permit(&h, "settings.write")?;
    let t = merchant(&a, &h)?;
    Ok(Json(
        rates::refresh(
            &a,
            &t,
            v["revision"].as_i64().ok_or(bad("Revision required"))?,
            &h,
        )
        .await?,
    ))
}
