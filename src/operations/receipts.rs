//! Idempotent immutable invoices/delivery notes with transactional per-shop number ranges.
use super::{master_data, receipt_pdf, receipt_text};
use crate::{
    App, IntoResponse, Json, Path, RequestContext, Response, Result, State, Value, auth, merchant,
    payments,
};
use veyndo_orders::store::receipts;
pub(super) async fn settings(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "documents.read")?;
    settings_value(&a, &h).await
}
pub(super) async fn settings_value(a: &App, h: &RequestContext) -> Result<Json<Value>> {
    master_data::read(a, h, None).await
}
pub(super) async fn save_settings(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "documents.create")?;
    auth::permit(&h, "settings.write")?;
    save_settings_value(&a, &h, v).await
}
pub(super) async fn save_settings_value(
    a: &App,
    h: &RequestContext,
    v: Value,
) -> Result<Json<Value>> {
    master_data::save(a, h, v, None).await
}
pub(super) async fn list(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "documents.read")?;
    auth::permit(&h, "customers.pii")?;
    Ok(Json(receipts::list(&a, &t, &id).await?))
}
pub(super) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "documents.create")?;
    auth::permit(&h, "customers.pii")?;
    Ok(Json(
        receipts::create(&PaymentInstructions, &a, &h, &t, &id, &v).await?,
    ))
}
/// Invoices print the transfer instructions the payments owner holds for the order.
struct PaymentInstructions;
impl veyndo_orders::ports::Payments for PaymentInstructions {
    fn document_instructions<'a>(
        &'a self,
        tx: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        order: &'a Value,
    ) -> veyndo_core::PortFuture<'a, Option<Value>> {
        Box::pin(payments::document_instructions(tx, tenant, order))
    }
}
pub(super) async fn pdf(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Response> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "documents.read")?;
    auth::permit(&h, "customers.pii")?;
    let (snapshot, content) = receipts::stored(&a, &t, &id).await?;
    Ok(super::document_response(&snapshot, content))
}
pub(crate) fn receipt_response(v: &Value) -> Response {
    (
        [
            ("content-type", "application/pdf"),
            ("cache-control", "private, no-store"),
            ("content-disposition", "attachment; filename=receipt.pdf"),
        ],
        receipt_pdf::render(&receipt_text::lines(v)),
    )
        .into_response()
}

/// Scoped immutable document export for separately consented adapters; never creates a new fiscal document.
pub(crate) async fn receipt_snapshot(
    a: &App,
    h: &RequestContext,
    order: &str,
    id: &str,
) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "documents.read")?;
    auth::permit(h, "orders.read")?;
    auth::permit(h, "customers.pii")?;
    receipts::snapshot(a, &t, order, id).await
}
