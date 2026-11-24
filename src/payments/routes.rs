//! Customer payment status/capture and merchant refund operations share the durable command API.
use super::{
    customer, enqueue, onboarding_route, provider_webhooks, providers_route, sessions, webhooks,
};
use crate::auth;
use crate::auth::SecureRoutes;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_core::shop_scope::tenant;

pub(crate) fn payment_router() -> Router<App> {
    Router::new()
        .route(
            "/store-api/account/payment-methods",
            get(super::instruments::route_list),
        )
        .route(
            "/store-api/account/payment-methods/{id}/revoke",
            post(super::instruments::route_revoke),
        )
        .route(
            "/store-api/payment-providers/{id}/webhooks",
            post(provider_webhooks::webhook),
        )
        .secure_route(
            "/api/payment-providers",
            &[("GET", "payments.read")],
            get(providers_route),
        )
        .secure_route(
            "/api/payment-providers/{id}/onboarding",
            &[("POST", "payments.manage")],
            post(onboarding_route),
        )
        .secure_route(
            "/api/payments/{id}/details",
            &[("GET", "payments.read")],
            get(super::details::route),
        )
        .secure_route("/api/payments", &[("GET", "payments.read")], get(status))
        .secure_route(
            "/api/payments/jobs/{id}",
            &[("GET", "payments.read")],
            get(veyndo_payments::http::status::job_status),
        )
        .secure_route(
            "/api/payments/{id}/instructions",
            &[("GET", "payments.read")],
            get(super::instructions::merchant_route),
        )
        .route(
            "/store-api/payments/{id}/instructions",
            get(super::instructions::customer_route),
        )
        .route("/store-api/payments/{id}", get(customer_status))
        .route("/store-api/payments/{id}/session", get(sessions::session))
        .route(
            "/store-api/payments/{id}/{operation}",
            post(customer_command),
        )
        .secure_route(
            "/api/payments/{id}/{operation}",
            &[("POST", "payments.manage")],
            post(merchant_command),
        )
        .route(
            "/store-api/payments/paypal/webhooks",
            post(webhooks::webhook),
        )
}
pub(crate) async fn status(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_payments::service::providers::status(&super::host::Gateway(&a), &a, &h).await?,
    ))
}
async fn customer_status(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    customer(&a, &h, &id).await?;
    Ok(Json(read(&a, &tenant(&h)?, &id).await?))
}
async fn customer_command(
    State(a): State<App>,
    h: RequestContext,
    Path((id, op)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    customer(&a, &h, &id).await?;
    if !["capture", "reconcile", "cancel"].contains(&op.as_str()) {
        return Err(bad("Unsupported customer payment operation"));
    }
    if ["amountMinor", "captureId", "finalCapture"]
        .iter()
        .any(|k| v.get(k).is_some())
    {
        return Err(bad(
            "Customer payment commands cannot change capture or refund allocations",
        ));
    }
    let status = read(&a, &tenant(&h)?, &id).await?;
    let op = if op == "capture" && status["intent"] == "authorize" {
        "authorize"
    } else {
        &op
    };
    Ok(Json(
        enqueue(
            &a,
            &h,
            &id,
            op,
            header(&h, "idempotency-key")
                .or_else(|| v["requestKey"].as_str())
                .ok_or(bad("Idempotency-Key required"))?,
            &v,
        )
        .await?,
    ))
}
async fn merchant_command(
    State(a): State<App>,
    h: RequestContext,
    Path((id, op)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "payments.manage")?;
    if !veyndo_payments::service::merchant_operations::admitted(&op, &v) {
        return Err(bad("Supported payment operation and approve=true required"));
    }
    Ok(Json(
        enqueue(
            &a,
            &h,
            &id,
            &op,
            header(&h, "idempotency-key")
                .or_else(|| v["requestKey"].as_str())
                .ok_or(bad("Idempotency-Key required"))?,
            &v,
        )
        .await?,
    ))
}

pub(crate) use veyndo_payments::http::status::read;
