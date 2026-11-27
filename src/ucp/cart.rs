//! UCP cart transfer adapts the native cart and single-use browser handoff; supplier totals and return URLs are never trusted.
use super::create_session;
use crate::cart_storage::load_cart;
use crate::checkout_handoff;
use crate::foundation::App;
use crate::ucp::UCP_VERSION;
use crate::ucp::document::{UcpCapability, UcpDocument};
use crate::ucp::profile::CHECKOUT;
use crate::ucp::views::{CheckoutHandoff, ShopLinks};
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::view;
use veyndo_core::request_context::RequestContext;
pub(crate) const CAPABILITY: &str = "dev.ucp.shopping.cart";
pub(super) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let mut doc = create_session(a.clone(), h.clone(), v, true).await?;
    let mut scoped = h;
    scoped.insert(
        "sw-context-token",
        doc.context_token.as_deref().unwrap().parse().unwrap(),
    );
    let c = load_cart(&a, &scoped).await?;
    let transfer: CheckoutHandoff = view(
        &checkout_handoff::issue(
            State(a.core.clone()),
            scoped,
            Json(json!({"revision":c.revision})),
        )
        .await?
        .0,
    );
    let url = reqwest::Url::parse(ShopLinks::of(&c.tenant).studio())
        .and_then(|u| u.join(transfer.checkout_path.as_deref().unwrap()))
        .map_err(|_| bad("Invalid cart continuation URL"))?;
    transfer_document(
        &mut doc,
        url.as_str(),
        (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
    );
    Ok(Json(doc.json()))
}
/// Turns a checkout document into the cart transfer document: no checkout token, payment, status or order.
fn transfer_document(doc: &mut UcpDocument, continue_url: &str, expires_at: String) {
    doc.context_token = None;
    doc.payment = None;
    doc.status = None;
    doc.order = None;
    doc.ucp.capabilities.remove(CHECKOUT);
    doc.ucp
        .capabilities
        .insert(CAPABILITY, UcpCapability::version(UCP_VERSION));
    doc.continue_url = Some(continue_url.to_owned());
    doc.expires_at = Some(expires_at);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ucp::document::tests::cart;
    use crate::ucp::ucp_document;

    #[test]
    fn transfer_document_matches_the_indexed_edits() {
        let quote = json!({"price":{"currency":"EUR"},"lineItems":[]});
        for c in [
            cart("open", json!({"email":"a@b.c"}), json!(null)),
            cart("completed", json!(null), json!({"id":"o1"})),
        ] {
            let mut typed = ucp_document(&c, &quote);
            typed.context_token = Some("token".into());
            let mut doc = typed.json();
            for field in ["context_token", "payment", "status", "order"] {
                doc.as_object_mut().unwrap().remove(field);
            }
            doc["ucp"]["capabilities"]
                .as_object_mut()
                .unwrap()
                .remove("dev.ucp.shopping.checkout");
            doc["ucp"]["capabilities"][CAPABILITY] = json!([{"version":UCP_VERSION}]);
            doc["continue_url"] = json!("https://studio.example/checkout/x");
            doc["expires_at"] = json!("2026-11-25T10:00:00+00:00");
            transfer_document(
                &mut typed,
                "https://studio.example/checkout/x",
                "2026-11-25T10:00:00+00:00".into(),
            );
            assert_eq!(typed.json(), doc);
        }
    }
}
