//! Version-pinned HMAC notifications enqueue reconciliation; external event payloads never write monetary state.
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;

use axum::body::Bytes;
pub(crate) async fn webhook(
    State(a): State<App>,
    h: RequestContext,
    Path(provider): Path<String>,
    bytes: Bytes,
) -> Result<Json<Value>> {
    if bytes.len() > 65536 {
        return Err(bad("Notification exceeds limit"));
    }
    let t = tenant(&h)?;
    Ok(Json(
        veyndo_payments::service::notifications::accept(
            &super::host::PaymentHost,
            &a,
            &h,
            &t,
            &provider,
            &bytes,
        )
        .await?,
    ))
}
