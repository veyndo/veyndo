//! The currency a storefront request asks for: the cart's own, else the request header.
use crate::{RequestContext, StoredCart, header};
pub(crate) fn requested(h: &RequestContext, cart: Option<&StoredCart>) -> String {
    cart.map(|c| c.data.currency.clone())
        .unwrap_or_else(|| header(h, "x-commerce-currency").unwrap_or("").into())
}
