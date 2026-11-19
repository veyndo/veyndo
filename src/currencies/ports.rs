//! The server's implementation of what the pricing component reads from the shop settings and the cart.
use crate::{App, RequestContext, commerce, load_cart};
use std::sync::Arc;
use veyndo_pricing::ports::{Carts, PortFuture, Pricing, ShopSettings};
use veyndo_pricing::types::currency::Config;

struct Server(App);

impl ShopSettings for Server {
    fn currencies<'a>(
        &'a self,
        tenant: &'a str,
        channel: &'a str,
    ) -> PortFuture<'a, (Config, i64)> {
        Box::pin(async move {
            let (settings, revision) = commerce::scoped_config(&self.0, tenant, channel).await?;
            Ok((settings.currencies.clone(), revision))
        })
    }
}

impl Carts for Server {
    fn currency<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, String> {
        Box::pin(async move { Ok(load_cart(&self.0, request).await?.data.currency) })
    }
}

impl axum::extract::FromRef<App> for Pricing {
    fn from_ref(app: &App) -> Self {
        let server = Arc::new(Server(app.clone()));
        Pricing {
            core: app.core.clone(),
            settings: server.clone(),
            carts: server,
        }
    }
}
