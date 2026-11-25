//! Actor-private manual-order quotes reuse native cart, legal, pricing, inventory and checkout through the assisted-selling register port; no second order ledger.
use crate::domain::views::Quote;
use crate::store::manual_orders as db;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_checkout::service::checkout_options;
use veyndo_checkout::store::cart_create::new_cart_context;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_intelligence::http::legal;
use veyndo_marketing::service::{channels, coupons};
use veyndo_pos::ports::Register;
use veyndo_pricing::domain::cart::Item;
use veyndo_shop::domain::cart_lines::validate as validate_items;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::settings::CheckoutSelection;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Draft {
    channel_id: String,
    customer_id: String,
    locale: String,
    currency: String,
    items: Vec<Item>,
    checkout: CheckoutSelection,
    #[serde(default)]
    codes: Vec<String>,
}
/// The merchant's order context on one sales channel: compound rights, the actor as order creator and no buyer session.
fn context(h: &RequestContext, channel: &str) -> Result<RequestContext> {
    merchant(h)?;
    for right in [
        "orders.write",
        "customers.read",
        "customers.pii",
        "catalog.read",
    ] {
        permit(h, right)?;
    }
    if !identifier(channel) {
        return Err(bad("Sales channel required"));
    }
    let mut h = h.clone();
    h.admin_order = Some(
        header(&h, "x-rac-user")
            .ok_or(bad("Merchant identity required"))?
            .into(),
    );
    h.insert(
        "sw-sales-channel-id",
        channel.parse().map_err(|_| bad("Invalid sales channel"))?,
    );
    h.remove("sw-context-token");
    h.remove("x-customer-token");
    Ok(h)
}
/// Builds the draft cart of a manual order for one customer and returns the quote to review.
async fn review(reg: &dyn Register, a: &State, mut h: RequestContext, v: &Value) -> Result<Value> {
    let draft: Draft = serde_json::from_value(v.clone())
        .map_err(|_| bad("Customer, items and checkout selection required"))?;
    validate_items(&draft.items)?;
    if draft.items.is_empty() {
        return Err(bad("Add at least one product"));
    }
    h.insert(
        "x-commerce-currency",
        draft
            .currency
            .parse()
            .map_err(|_| bad("Invalid currency"))?,
    );
    let row = db::active_customer(a, &tenant(&h)?, &draft.customer_id)
        .await?
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "Active customer unavailable".into(),
        ))?;
    let mut c = new_cart_context(a, &h, "", "admin").await?;
    c.data.admin_actor = h.admin_order.clone();
    c.data.customer_id = Some(row.id);
    c.data.email = Some(row.email);
    c.data.group = row.group;
    c.data.company = row.company;
    db::set_cart_data(a, &c.tenant, &c.id, &json!(c.data)).await?;
    h.insert("sw-context-token", c.token.parse().unwrap());
    c = reg
        .carts()
        .replace(&h, draft.items, Some(c.revision))
        .await?;
    let mut selection = draft.checkout;
    if selection.billing_address_id.is_some() || selection.shipping_address_id.is_some() {
        return Err(bad(
            "Supply the reviewed address snapshot, not account address IDs",
        ));
    }
    selection.customer_email = c.data.email.clone();
    let quote = reg
        .select(&h, json!({"revision":c.revision,"checkout":selection}))
        .await?;
    let c = reg.carts().load(&h).await?;
    let quote = if draft.codes.is_empty() {
        quote
    } else {
        coupons::select(
            reg.carts(),
            a,
            &h,
            &json!({"revision":c.revision,"codes":draft.codes}),
        )
        .await?
    };
    if !matches!(
        view::<Quote>(&quote).payment_method.mode.as_deref(),
        Some("simulated" | "manual")
    ) {
        return Err(bad(
            "Manual orders require a configured manual or simulated payment; real provider payment must be completed by the buyer",
        ));
    }
    Ok(
        json!({"cart":quote,"policy":legal::invoke(a,&h,"privacy.policy",&json!({})).await?,"channelId":draft.channel_id,"locale":draft.locale}),
    )
}
/// The input of a manual order step; `complete` carries the reviewed cart, revision, exact total and consent.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ManualOrderStep {
    #[serde(deserialize_with = "lenient")]
    channel_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    cart_token: Option<String>,
    #[serde(deserialize_with = "lenient")]
    customer_acknowledgement: Option<bool>,
    policy_version: Value,
    terms: Value,
    digital_immediate: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    total_minor: Option<String>,
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
}

/// One manual order step: `context`, `review` of a draft cart, or the approved `complete`.
pub async fn invoke(
    reg: &dyn Register,
    a: &State,
    original: &RequestContext,
    operation: &str,
    v: &Value,
) -> Result<Value> {
    let input: ManualOrderStep = view(v);
    let channel = input
        .channel_id
        .as_deref()
        .ok_or(bad("Sales channel required"))?;
    let mut h = context(original, channel)?;
    let (cfg, _) = settings(a, &tenant(&h)?, channel).await?;
    let locale = input.locale.as_deref().unwrap_or(&cfg.main_locale);
    h.insert(
        "x-commerce-locale",
        locale.parse().map_err(|_| bad("Invalid order language"))?,
    );
    if !cfg.locales.iter().any(|l| l == locale) {
        return Err(bad("Order language unavailable"));
    }
    let data = channels::channel(a, &h, channel, locale).await?;
    if data
        .as_ref()
        .is_some_and(|c| !c.active || c.visibility != "public")
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sales channel is paused or private; open an authorized merchant preview".into(),
        ));
    }
    if data.is_some_and(|c| c.kind != "storefront" && c.kind != "headless") {
        return Err(bad(
            "Choose a storefront or API sales channel; use the dedicated POS or marketplace workflow otherwise",
        ));
    }
    if operation == "context" {
        let channels = db::channels(a, &tenant(&h)?).await?;
        return Ok(
            json!({"options":checkout_options::options(a,reg.quotes(),&h).await?,"currencies":reg.currency(&h,"currency.list",&json!({})).await?,"policy":legal::invoke(a,&h,"privacy.policy",&json!({})).await?,"channels":channels,"customerGroups":cfg.customer_groups,"mainLocale":cfg.main_locale}),
        );
    }
    if operation == "review" {
        return Box::pin(review(reg, a, h, v)).await;
    }
    if operation != "complete" || input.approve != Some(true) {
        return Err(bad("Explicit reviewed order approval required"));
    }
    h.insert(
        "sw-context-token",
        input
            .cart_token
            .as_deref()
            .ok_or(bad("Reviewed cart required"))?
            .parse()
            .map_err(|_| bad("Invalid cart token"))?,
    );
    let c = reg.carts().load(&h).await?;
    if c.data.admin_actor.is_none() {
        return Err(bad("Not an admin order draft"));
    }
    if c.data.locale != locale {
        return Err(conflict("Order language changed; review again"));
    }
    if c.status == "open" {
        let quote = reg.carts().view(&c).await?;
        if !matches!(
            view::<Quote>(&quote).payment_method.mode.as_deref(),
            Some("simulated" | "manual")
        ) {
            return Err(bad("Buyer payment required for this provider"));
        }
        if input.customer_acknowledgement != Some(true) {
            return Err(bad(
                "Confirm the customer's authorization before creating the order",
            ));
        }
        legal::invoke(a,&h,"legal.accept",&json!({"policyVersion":input.policy_version,"terms":input.terms,"digitalImmediate":input.digital_immediate})).await?;
    }
    h.insert(
        "x-commerce-cart-revision",
        input
            .revision
            .ok_or(bad("Reviewed revision required"))?
            .to_string()
            .parse()
            .unwrap(),
    );
    h.insert(
        "x-commerce-total-minor",
        input
            .total_minor
            .as_deref()
            .ok_or(bad("Reviewed exact total required"))?
            .parse()
            .map_err(|_| bad("Invalid exact total"))?,
    );
    Box::pin(
        reg.checkout(
            &h,
            input
                .request_key
                .as_deref()
                .ok_or(bad("Order request key required"))?,
        ),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::context;
    use veyndo_core::RequestContext;

    #[test]
    fn the_order_context_needs_a_merchant_and_a_valid_channel() {
        assert!(context(&RequestContext::default(), "web").is_err());
    }
}
