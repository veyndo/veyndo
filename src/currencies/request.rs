//! The currency a storefront request asks for: the cart's own, else the request header.
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_shop::types::cart::StoredCart;
pub(crate) fn requested(h: &RequestContext, cart: Option<&StoredCart>) -> String {
    cart.map(|c| c.data.currency.clone())
        .unwrap_or_else(|| header(h, "x-commerce-currency").unwrap_or("").into())
}
