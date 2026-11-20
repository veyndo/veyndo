//! Version-pinned HMAC notifications enqueue reconciliation; external event payloads never write monetary state.
use crate::{App, Json, Path, RequestContext, Result, State, Value, bad, tenant};

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
