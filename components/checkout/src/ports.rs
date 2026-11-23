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
    /// The provider and method pairs the installed payment providers offer for the currency.
    fn available<'a>(
        &'a self,
        tenant: &'a str,
        channel: &'a str,
        currency: &'a str,
    ) -> PortFuture<'a, std::collections::HashSet<(String, String)>>;
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

/// What a cart edit needs from the server around the native revision check: app configurations, protocol consent and the agent run fence.
pub trait CartEdits: Send + Sync {
    /// The priced cart as the storefront receives it.
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value>;
    /// Applies the edit's app configuration selections to the candidate before it is priced.
    fn configure<'a>(&'a self, cart: &'a mut StoredCart) -> PortFuture<'a, ()>;
    /// Checks the configurations under the cart lock, before the new revision is written.
    fn locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
    ) -> PortFuture<'a, ()>;
    /// Configuration events, consent and the agent run fence, in the transaction that wrote the revision.
    fn committed<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
    ) -> PortFuture<'a, ()>;
}
