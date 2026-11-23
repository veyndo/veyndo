//! Customer-bound, short-lived provider UI sessions; iframe messages are never ledger receipts.
use crate::{App, Json, Path, RequestContext, Result, State, Value};

pub(crate) async fn session(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    axum::extract::Query(input): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_payments::service::sessions::session(
            &super::host::PaymentHost,
            &super::host::Gateway(&a),
            &a,
            &h,
            &id,
            input.get("parentOrigin").map(String::as_str),
        )
        .await?,
    ))
}
