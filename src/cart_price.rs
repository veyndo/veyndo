//! Authoritative quantity pricing and localized checkout quote assembly.
use crate::*;

pub(crate) use veyndo_checkout::domain::cart_quote::{normalized_quantity, quote};
pub(crate) async fn cart_json(a: &App, c: &StoredCart) -> Result<Value> {
    project_cart(a, c, false).await
}

pub(crate) async fn project_cart(
    a: &App,
    c: &StoredCart,
    include_policy_version: bool,
) -> Result<Value> {
    veyndo_checkout::service::cart_projection::project(
        &a.core,
        &Quotes(a),
        c,
        include_policy_version,
    )
    .await
}

/// The server side of the checkout quote: catalog reads, provider payment methods, app hooks and the cashier discount.
pub(crate) struct Quotes<'a>(pub(crate) &'a App);
impl veyndo_checkout::ports::Quotes for Quotes<'_> {
    fn products<'a>(
        &'a self,
        tenant: &'a str,
        chain: &'a [String],
        items: &'a [Item],
    ) -> veyndo_core::PortFuture<'a, Vec<Product>> {
        Box::pin(commerce::cart_products(self.0, tenant, chain, items))
    }
    fn methods<'a>(
        &'a self,
        tenant: &'a str,
        channel: &'a str,
        currency: &'a str,
        settings: commerce::Settings,
    ) -> veyndo_core::PortFuture<'a, (commerce::Settings, Option<String>)> {
        Box::pin(payments::currency_methods(
            self.0, tenant, channel, currency, settings,
        ))
    }
    fn available<'a>(
        &'a self,
        tenant: &'a str,
        channel: &'a str,
        currency: &'a str,
    ) -> veyndo_core::PortFuture<'a, std::collections::HashSet<(String, String)>> {
        Box::pin(payments::registry::available(
            self.0, tenant, channel, currency,
        ))
    }
    fn adjust<'a>(
        &'a self,
        cart: &'a StoredCart,
        preview: &'a StoredCart,
        products: &'a [Product],
        quote: Value,
        settings: &'a commerce::Settings,
        revision: i64,
    ) -> veyndo_core::PortFuture<'a, Value> {
        Box::pin(async move {
            let hooks = apps::prepare_hooks(self.0, &cart.tenant).await?;
            let result = apps::apply_hooks(
                &mut *self.0.db.begin().await?,
                preview,
                products,
                quote,
                settings,
                revision,
                &hooks,
            )
            .await?;
            pos::adjust_quote(cart, result, products, settings, revision)
        })
    }
}
