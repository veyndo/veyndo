//! Cashier-only customer lookup/card resolution and revision-checked account attachment; never issues customer credentials.
use crate::service::guard;
use crate::types::events::CustomerSelected;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_checkout::store::carts::{self, CartRow};
use veyndo_checkout::store::register_carts;
use veyndo_core::document::view;
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_customers::service::address_book;
use veyndo_customers::store::cashier as customers;
use veyndo_identity::permit;
use veyndo_shop::ports::Carts;
use veyndo_shop::types::cart::selection;

pub async fn find(a: &State, h: &RequestContext, query: &str, card: Option<&str>) -> Result<Value> {
    permit(h, "customers.read")?;
    permit(h, "customers.pii")?;
    if query.len() > 200 || card.is_some_and(|s| s.is_empty() || s.len() > 100) {
        return Err(bad("Invalid customer search or card"));
    }
    Ok(json!({"elements":customers::search(a, &tenant(h)?, query, card).await?}))
}
/// Whether the priced cart still asks for its checkout selection to be confirmed.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SelectionReview {
    selection_needs_confirmation: Value,
}

pub async fn select(
    carts: &dyn Carts,
    a: &State,
    h: &RequestContext,
    id: Option<&str>,
    revision: i64,
) -> Result<Value> {
    permit(h, "customers.read")?;
    permit(h, "customers.pii")?;
    permit(h, "orders.write")?;
    let mut tx = a.db.begin().await?;
    let mut c = carts::lock_by_token(&mut tx, &tenant(h)?, token(h)?)
        .await?
        .map(CartRow::into_cart)
        .transpose()?
        .ok_or(bad("Cart unavailable"))?;
    guard::cart(h, &c)?;
    if c.data.pos_actor.is_none() || c.status != "open" || c.revision != revision {
        return Err(conflict(
            "POS sale changed; reload before selecting customer",
        ));
    }
    let mut selection = selection(&c.data);
    selection.billing_address_id = None;
    selection.shipping_address_id = None;
    selection.billing_address = None;
    selection.address = None;
    c.data.customer_id = None;
    c.data.email = None;
    c.data.company = None;
    c.data.group = "consumer".into();
    selection.customer_email = None;
    if let Some(id) = id {
        let account = customers::account(&mut tx, &c.tenant, id).await?;
        if let Some(address) = &account.default_billing_address_id {
            selection.billing_address =
                Some(address_book::get(&mut tx, &c.tenant, &account.email, address).await?);
        }
        c.data.customer_id = Some(id.into());
        c.data.email = Some(account.email.clone());
        c.data.company = account.company;
        c.data.group = account.group;
        selection.customer_email = Some(account.email);
    }
    c.data.checkout = Some(selection);
    c.revision += 1;
    let quote = carts.view(&c).await?;
    if view::<SelectionReview>(&quote).selection_needs_confirmation == true {
        return Err(bad(
            "Customer pricing requires a different available payment; change payment first",
        ));
    }
    register_carts::save(&mut tx, &json!(c.data), c.revision, &c.tenant, &c.id).await?;
    let selected = CustomerSelected {
        cart_id: &c.id,
        customer_id: id,
        cashier_id: h.pos.as_ref().map(|p| p.actor.as_str()),
        sales_channel_id: &c.data.sales_channel,
    };
    veyndo_core::events::emit(&mut *tx, &c.tenant, &selected).await?;
    tx.commit().await?;
    Ok(quote)
}
