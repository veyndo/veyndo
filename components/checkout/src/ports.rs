//! What the cart projection needs from the server: localized cart products, the payment methods providers admit, and the app hooks and cashier discount that run after native pricing.
use serde_json::Value;
use veyndo_core::PortFuture;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::domain::cart::Item;
use veyndo_shop::Settings;
use veyndo_shop::types::cart::StoredCart;

/// The server side of a cart quote; the native calculation itself stays in this component.
pub trait Quotes: Send + Sync {
    /// The products of the cart lines, translated along the language chain.
    fn products<'a>(
        &'a self,
        tenant: &'a str,
        chain: &'a [String],
        items: &'a [Item],
    ) -> PortFuture<'a, Vec<Product>>;
    /// The settings with only the payment methods the installed providers admit for the currency, plus the PayPal environment.
    fn methods<'a>(
        &'a self,
        tenant: &'a str,
        channel: &'a str,
        currency: &'a str,
        settings: Settings,
    ) -> PortFuture<'a, (Settings, Option<String>)>;
    /// App commerce hooks on the previewed cart, then the cashier discount on the stored cart.
    fn adjust<'a>(
        &'a self,
        cart: &'a StoredCart,
        preview: &'a StoredCart,
        products: &'a [Product],
        quote: Value,
        settings: &'a Settings,
        revision: i64,
    ) -> PortFuture<'a, Value>;
}
