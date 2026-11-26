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

#[cfg(test)]
mod tests {
    use super::{admin, request};
    use crate::types::cart::StoredCart;
    use serde_json::{Value, json};
    use veyndo_core::request_context::PosAdmission;
    use veyndo_core::{RequestContext, StatusCode};

    fn cart(extra: Value) -> StoredCart {
        let mut data = json!({"items":[],"group":"retail","session":"s","email":null,"company":null,"buyer":null,"order":null});
        for (key, value) in extra.as_object().unwrap() {
            data[key] = value.clone();
        }
        StoredCart {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: serde_json::from_value(data).unwrap(),
            revision: 1,
            status: "open".into(),
        }
    }

    fn merchant(user: &'static str) -> RequestContext {
        let mut h = RequestContext::new();
        h.insert("x-rac-user", user.parse().unwrap());
        h.insert("x-rac-role", "admin".parse().unwrap());
        h
    }

    #[test]
    fn a_manual_order_cart_admits_only_its_creating_merchant() {
        let c = cart(json!({"admin_actor":"merchant-1"}));
        let mut h = merchant("merchant-1");
        assert_eq!(
            admin(&h, &c).err().map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        h.admin_order = Some("merchant-1".into());
        assert!(admin(&h, &c).is_ok());
        assert!(request(&h, &c).is_ok());
        let mut other = merchant("merchant-2");
        other.admin_order = Some("merchant-1".into());
        assert!(admin(&other, &c).is_err());
        let mut viewer = RequestContext::new();
        viewer.insert("x-rac-user", "merchant-1".parse().unwrap());
        viewer.insert("x-rac-role", "viewer".parse().unwrap());
        viewer.admin_order = Some("merchant-1".into());
        assert!(admin(&viewer, &c).is_err());
        assert!(admin(&RequestContext::new(), &cart(json!({}))).is_ok());
    }

    #[test]
    fn a_pos_cart_needs_its_cashier_and_every_cart_its_own_channel() {
        let c = cart(json!({"pos_actor":"cashier-1"}));
        let mut h = RequestContext::new();
        assert_eq!(
            request(&h, &c).err().map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        h.pos = Some(PosAdmission {
            actor: "cashier-2".into(),
            tender_minor: None,
        });
        assert!(request(&h, &c).is_err());
        h.pos = Some(PosAdmission {
            actor: "cashier-1".into(),
            tender_minor: None,
        });
        assert!(request(&h, &c).is_ok());
        let with_customer = cart(json!({"pos_actor":"cashier-1","customer_id":"customer-1"}));
        assert!(request(&h, &with_customer).is_err());
        let mut cashier = merchant("cashier-1");
        cashier.pos = h.pos.clone();
        assert!(request(&cashier, &with_customer).is_ok());

        let plain = cart(json!({}));
        let mut visitor = RequestContext::new();
        assert!(request(&visitor, &plain).is_ok());
        visitor.insert("sw-sales-channel-id", "outlet".parse().unwrap());
        assert_eq!(
            request(&visitor, &plain).err().map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        assert!(request(&visitor, &cart(json!({"sales_channel":"outlet"}))).is_ok());
    }
}
