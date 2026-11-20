//! Who may touch a cart: a merchant-created cart only its creating merchant, a POS cart only its admitted cashier, and every cart only its own sales channel.
use crate::types::cart::StoredCart;
use veyndo_core::shop_scope::{channel, header};
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_identity::permit;

pub fn admin(h: &RequestContext, c: &StoredCart) -> Result<()> {
    if let Some(actor) = &c.data.admin_actor {
        if h.admin_order.as_ref() != Some(actor) || header(h, "x-rac-user") != Some(actor.as_str())
        {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Manual order requires its creating merchant".into(),
            ));
        }
        for right in [
            "orders.write",
            "customers.read",
            "customers.pii",
            "catalog.read",
        ] {
            permit(h, right)?;
        }
    }
    Ok(())
}

/// Runs before every cart read or write. A stolen POS token is neither merchant nor channel admission.
pub fn request(h: &RequestContext, c: &StoredCart) -> Result<()> {
    admin(h, c)?;
    if c.data.pos_actor.is_some()
        && !h
            .pos
            .as_ref()
            .is_some_and(|p| c.data.pos_actor.as_deref() == Some(&p.actor))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS cart belongs to another cashier or requires POS admission".into(),
        ));
    }
    if c.data.sales_channel != channel(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cart belongs to another sales channel".into(),
        ));
    }
    if c.data.pos_actor.is_some() && c.data.customer_id.is_some() {
        permit(h, "customers.read")?;
        permit(h, "customers.pii")?;
    }
    Ok(())
}
