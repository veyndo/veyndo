//! Immutable private carrier documents belong to the existing order receipt store, never product assets.
use super::receipts;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{apps, assets, auth};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde_json::Value;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
pub(crate) use veyndo_orders::store::carrier_documents::allocation_digest;
use veyndo_orders::store::carrier_documents::{Document, record};
pub(crate) async fn import_document(
    a: &App,
    h: &RequestContext,
    m: &apps::Manifest,
    metadata: Value,
    content: Vec<u8>,
) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "documents.create")?;
    auth::permit(h, "orders.write")?;
    auth::permit(h, "customers.pii")?;
    if h.principal.app.as_deref() != Some(m.id.as_str()) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Carrier documents require the approved app callback identity".into(),
        ));
    }
    if veyndo_core::store::environments::parent(a, &t)
        .await?
        .is_some()
    {
        return Err(bad("Operational documents cannot be staged"));
    }
    let document = Document::admit(&m.id, &metadata, &content, &assets::safe_url)?;
    let mut tx = a.db.begin().await?;
    veyndo_core::store::history_context::context(&mut tx, h, "app").await?;
    apps::lock_package_write(&mut tx, &t, m).await?;
    let result = record(&mut tx, &t, h, &m.id, &m.version, document, content).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) fn document_response(snapshot: &Value, content: Option<Vec<u8>>) -> Response {
    if let Some(content) = content {
        return (
            [
                ("content-type", "application/pdf"),
                ("cache-control", "private, no-store"),
                ("x-content-type-options", "nosniff"),
                (
                    "content-disposition",
                    "attachment; filename=carrier-document.pdf",
                ),
            ],
            content,
        )
            .into_response();
    }
    receipts::receipt_response(snapshot)
}
