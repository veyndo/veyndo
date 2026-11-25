//! Cashier-only customer lookup/card resolution and revision-checked account attachment; never issues customer credentials.
use crate::service::guard;
use crate::store::carts as cart_rows;
use crate::store::customers;
use serde_json::{Value, json};
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_customers::service::address_book;
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
    let mut c = cart_rows::lock(&mut tx, &tenant(h)?, token(h)?)
        .await?
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
    if quote["selectionNeedsConfirmation"] == true {
        return Err(bad(
            "Customer pricing requires a different available payment; change payment first",
        ));
    }
    cart_rows::save(&mut tx, &json!(c.data), c.revision, &c.tenant, &c.id).await?;
    veyndo_core::store::outbox::publish_on(&mut *tx, &c.tenant, "pos.customer_selected", &json!({"cartId":c.id,"customerId":id,"cashierId":h.pos.as_ref().map(|p|&p.actor),"salesChannelId":c.data.sales_channel})).await?;
    tx.commit().await?;
    Ok(quote)
}
