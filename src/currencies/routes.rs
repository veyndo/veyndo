//! Native currency discovery, revision-safe selection and authenticated FX/price job operations.
use super::rates;
use crate::auth::SecureRoutes;
use crate::cart_price::cart_json;
use crate::cart_storage::load_cart;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{auth, commerce};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
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
/// The JSON fields `select` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SelectInput {
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

pub(super) async fn select(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SelectInput = view(&v);
    let current = load_cart(&a, &h).await?;
    let (s, _) = commerce::scoped_config(&a, &current.tenant, &current.data.sales_channel).await?;
    let code = s
        .currencies
        .selected(input.currency.as_deref().ok_or(bad("Currency required"))?)?
        .code
        .clone();
    let mut c = current.clone();
    c.data.currency = code;
    if c.status != "open" || input.revision != Some(c.revision) {
        return Err(conflict("Cart changed or terminal"));
    }
    c.revision += 1;
    let q = cart_json(&a, &c).await?;
    if !veyndo_checkout::store::cart_load::replace_open(&a, &c, current.revision).await? {
        return Err(conflict("Cart changed; reload first"));
    }
    Ok(Json(q))
}
/// The JSON fields `refresh` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RefreshInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

pub(super) async fn refresh(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: RefreshInput = view(&v);
    auth::permit(&h, "settings.write")?;
    let t = merchant(&a, &h)?;
    Ok(Json(
        rates::refresh(&a, &t, input.revision.ok_or(bad("Revision required"))?, &h).await?,
    ))
}
