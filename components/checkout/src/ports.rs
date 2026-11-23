//! What checkout needs from the server: cart products, provider payment methods, app hooks and configurations, the cashier, the built-in payment provider and the agent run fence.
use serde_json::Value;
use veyndo_core::{PortFuture, RequestContext, Result};
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

/// One cart line set priced under the purchase locks, as the server-side adjustments receive it.
pub struct LockedQuote<'a> {
    pub cart: &'a StoredCart,
    pub products: &'a [Product],
    pub settings: &'a Settings,
    pub revision: i64,
}

/// What the purchase transaction needs from the server: app sandboxes and hooks, the cashier, payment providers and the agent run fence.
pub trait Purchase: Send + Sync {
    /// App configuration modules, commerce hooks and, for a business cart, the tenant policy, prepared before any lock.
    type Prepared: Send + Sync;
    /// Prepares the app modules of the admitted cart.
    fn prepare<'a>(
        &'a self,
        cart: &'a StoredCart,
        business: bool,
    ) -> PortFuture<'a, Self::Prepared>;
    /// The payment methods the providers admit, read on the purchase transaction.
    fn methods_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
        currency: &'a str,
        settings: Settings,
    ) -> PortFuture<'a, Settings>;
    /// The cashier's register checks of a POS purchase under the locks.
    fn register_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        h: &'a RequestContext,
        cart: &'a StoredCart,
    ) -> PortFuture<'a, ()>;
    /// Checks the cart's app configurations against the prepared modules.
    fn configurations_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
        prepared: &'a Self::Prepared,
    ) -> PortFuture<'a, ()>;
    /// App commerce hooks, then the cashier discount, on the locked quote.
    fn adjust_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        locked: LockedQuote<'a>,
        quote: Value,
        prepared: &'a Self::Prepared,
    ) -> PortFuture<'a, Value>;
    /// The prepared tenant policy approves a business purchase at the current extension digest.
    fn approve(&self, prepared: &Self::Prepared, digest: &str, minor: i64) -> Result<()>;
    /// The cash tender and settlement rules of a POS purchase.
    fn settlement(
        &self,
        h: &RequestContext,
        cart: &StoredCart,
        quote: &Value,
        settings: &Settings,
        minor: i64,
    ) -> Result<()>;
    /// The built-in provider is configured for the selected method's environment.
    fn provider_ready(&self, tenant: &str, payment_method: &str) -> Result<()>;
    /// Binds the order to the native lifecycle of its apps.
    fn bind_order<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        order: &'a mut Value,
    ) -> PortFuture<'a, ()>;
    /// The cashier's settlement annotation of a POS order.
    fn settle<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        h: &'a RequestContext,
        cart: &'a StoredCart,
        order: &'a mut Value,
    ) -> PortFuture<'a, ()>;
    /// Prepares the provider payment of an order paid outside the shop.
    fn prepare_payment<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
        order: &'a mut Value,
        minor: i64,
        return_origin: Option<&'a str>,
    ) -> PortFuture<'a, ()>;
    /// Joins the commit to a running agent run, or does nothing outside one.
    fn fence<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
    ) -> PortFuture<'a, ()>;
}
