//! UCP B2B commands route to native RFQ/CAS/approval owners; a checkout token alone never identifies an employee.
use super::document::{UcpCapability, UcpDocument, UcpMessage, UcpNegotiation};
use super::views::{CheckoutHandoff, OrderSnapshot, PricedCart, Purchasing, QuotePage, ShopLinks};
use super::{consent, profile, ucp_document, ucp_load};
use crate::cart_price::cart_json;
use crate::foundation::App;
use crate::{accounts, b2b, checkout_handoff};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::view;
use veyndo_core::request_context::RequestContext;
use veyndo_shop::types::cart::StoredCart;
fn operation(op: &str) -> Result<String> {
    if op == "checkout.handoff" {
        return Ok(op.into());
    }
    if ![
        "contexts",
        "select",
        "quote.current",
        "quote.get",
        "quote.request",
        "quote.message",
        "quote.reject",
        "quote.accept",
        "quote.release",
        "approval.request",
    ]
    .contains(&op)
    {
        return Err(bad(
            "Unsupported buyer negotiation operation; merchant offers and approver decisions use their authenticated native APIs",
        ));
    }
    Ok(format!("b2b.{op}"))
}
/// The JSON fields `command` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CommandInput {
    id: Value,
}

pub(crate) async fn command(
    State(a): State<App>,
    h: RequestContext,
    Path((id, op)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CommandInput = view(&v);
    let name = operation(&op)?;
    let selection = profile::select(&h, false).await?;
    if !selection.b2b {
        return Err(conflict(
            "capabilities_incompatible: negotiate ai.veyndo.app.shopping.b2b_negotiation first",
        ));
    }
    let c = ucp_load(&a, &h, &id).await?;
    // Guard session/membership even on read or closed-cart paths; native commands repeat checks under locks.
    b2b::guard(&a, &h, &c).await?;
    accounts::identity(&a, &h).await?;
    if ["quote.message", "quote.reject"].contains(&op.as_str()) {
        Box::pin(b2b::invoke(
            &a,
            &h,
            "b2b.quote.get",
            &json!({"id":input.id}),
        ))
        .await?;
    }
    let result = if name == "checkout.handoff" {
        if v.as_object()
            .is_none_or(|o| o.len() != 1 || !o.contains_key("revision"))
        {
            return Err(bad(
                "Checkout handoff requires only the current cart revision",
            ));
        }
        checkout_handoff::issue(State(a.core.clone()), h.clone(), Json(v))
            .await?
            .0
    } else {
        // Keep the large shared domain dispatcher out of the UCP future's stack frame.
        Box::pin(b2b::invoke(&a, &h, &name, &v)).await?
    };
    let current = ucp_load(&a, &h, &id).await?;
    let mut doc = response(&a, &h, &current, profile::select(&h, false).await?).await?;
    if name == "checkout.handoff" {
        let handoff: CheckoutHandoff = view(&result);
        let url = reqwest::Url::parse(ShopLinks::of(&c.tenant).studio())
            .and_then(|u| u.join(handoff.checkout_path.as_deref().unwrap()))
            .map_err(|_| bad("Invalid checkout continuation URL"))?;
        doc.continue_url = Some(url.as_str().to_owned());
    }
    doc.b2b_negotiation.get_or_insert_default().result = Some(result);
    Ok(Json(doc.json()))
}
pub(super) async fn response(
    a: &App,
    h: &RequestContext,
    c: &StoredCart,
    selected: profile::Selection,
) -> Result<UcpDocument> {
    let mut quote = cart_json(a, c).await?;
    // Completed price snapshots stay immutable; purchasing metadata describes current policy.
    if selected.b2b && c.data.b2b.is_some() && view::<PricedCart>(&quote).b2b.is_null() {
        b2b::preview(a, c, &mut quote).await?;
    }
    let mut doc = ucp_document(c, &quote);
    if selected.consent {
        consent::project(a, h, c, &mut doc).await?;
    } else if let Some(buyer) = doc.buyer.as_mut().and_then(Value::as_object_mut) {
        buyer.remove("consent");
    }
    // Keep canonical tenant/channel links instead of localhost-only continuation URLs.
    let mut url = reqwest::Url::parse(ShopLinks::of(&c.tenant).storefront())
        .map_err(|_| bad("Invalid shop origin"))?;
    url.query_pairs_mut()
        .append_pair("channel", &c.data.sales_channel);
    url.set_fragment(Some("checkout"));
    doc.continue_url = Some(url.as_str().to_owned());
    if let Some(order) = c.data.order.as_ref().map(view::<OrderSnapshot>) {
        url.set_fragment(Some(&format!(
            "order/{}",
            order.id.as_str().unwrap_or_default()
        )));
        if let Some(link) = doc.order.as_mut() {
            link.permalink_url = Some(url.as_str().to_owned());
        }
        if let Some(attempt) = &order.payment.attempt_id
            && doc.status == Some("requires_escalation")
        {
            url.set_fragment(Some(&format!("payment/{attempt}")));
            doc.continue_url = Some(url.as_str().to_owned());
        }
    }
    if !selected.b2b {
        return Ok(doc);
    }
    doc.ucp
        .capabilities
        .insert(profile::B2B, UcpCapability::version(profile::VERSION));
    let quote: PricedCart = view(&quote);
    let procurement = quote.b2b;
    let purchasing: Purchasing = view(&procurement);
    let mut negotiation = UcpNegotiation {
        checkout_revision: Some(c.revision),
        purchasing: Some(procurement.clone()),
        quotes: Some(json!([])),
        ..UcpNegotiation::default()
    };
    if c.data.b2b.is_some() && purchasing.quotes_allowed == Some(true) {
        let quotes: QuotePage =
            view(&Box::pin(b2b::invoke(a, h, "b2b.quote.current", &json!({}))).await?);
        negotiation.quotes = Some(quotes.elements);
        negotiation.quote_limit = Some(quotes.limit);
    }
    doc.b2b_negotiation = Some(negotiation);
    if c.status == "open"
        && let Some((code, _)) = blocker(&procurement)
    {
        doc.status = Some("requires_escalation");
        doc.messages = vec![UcpMessage::info(code, &c.data.locale)];
    }
    Ok(doc)
}
fn blocker(p: &Value) -> Option<(&'static str, &'static str)> {
    let p: Purchasing = view(p);
    if p.budget_available == Some(false) {
        Some((
            "b2b_budget_exceeded",
            "Purchasing budget exceeded. Ask your company administrator.",
        ))
    } else if p.purchase_order_required == Some(true)
        && p.purchase_order
            .as_deref()
            .is_none_or(|v| v.trim().is_empty())
    {
        Some((
            "b2b_purchase_order_required",
            "Enter the purchase-order reference required by your company.",
        ))
    } else if p.approval_required == Some(true) && p.approval.state.as_deref() != Some("approved") {
        Some((
            "b2b_approval_required",
            "The current negotiated checkout requires a separate authorized purchasing approver.",
        ))
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiation_block_matches_the_indexed_shape() {
        let purchasing = json!({"quotesAllowed":true});
        let mut block = UcpNegotiation {
            checkout_revision: Some(3),
            purchasing: Some(purchasing.clone()),
            quotes: Some(json!([])),
            ..UcpNegotiation::default()
        };
        let mut indexed = json!({"checkout_revision":3,"purchasing":purchasing,"quotes":[]});
        assert_eq!(serde_json::to_value(&block).unwrap(), indexed);
        let page: QuotePage = view(&json!({"elements":[{"id":"q"}]}));
        block.quotes = Some(page.elements);
        block.quote_limit = Some(page.limit);
        block.result = Some(json!({"ok":true}));
        indexed["quotes"] = json!([{"id":"q"}]);
        indexed["quote_limit"] = Value::Null;
        indexed["result"] = json!({"ok":true});
        assert_eq!(serde_json::to_value(&block).unwrap(), indexed);
        let mut fresh: Option<UcpNegotiation> = None;
        fresh.get_or_insert_default().result = Some(json!(1));
        assert_eq!(serde_json::to_value(fresh).unwrap(), json!({"result":1}));
    }
    #[test]
    fn buyer_protocol_cannot_offer_or_self_grant_merchant_authority() {
        assert!(operation("quote.request").is_ok());
        for name in [
            "quote.offer",
            "approval.decide",
            "company.save",
            "../../api/b2b/quote.offer",
        ] {
            assert!(operation(name).is_err());
        }
    }
    #[test]
    fn readiness_describes_current_budget_and_separate_approval() {
        assert_eq!(
            blocker(&json!({"budgetAvailable":false})).unwrap().0,
            "b2b_budget_exceeded"
        );
        assert_eq!(
            blocker(&json!({"approvalRequired":true,"approval":{"state":"requested"}}))
                .unwrap()
                .0,
            "b2b_approval_required"
        );
        assert!(blocker(&json!({"budgetAvailable":true,"approvalRequired":true,"approval":{"state":"approved"}})).is_none());
    }
}
