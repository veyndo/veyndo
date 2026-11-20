//! Exchange a native buyer login for short-lived, Checkout-only authority.
pub(crate) use veyndo_checkout::http::buyer_identity::{
    INTROSPECTION, admitted_shop, bind, browser_checkout_transport, inspect, issue,
};
