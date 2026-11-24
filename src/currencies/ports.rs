//! The server's implementation of what the pricing component reads from the shop settings, the cart and the catalogue.
use crate::cart_storage::load_cart;
use crate::commerce;
use crate::foundation::App;
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::ports::{Carts, Catalogue, PortFuture, Pricing, ShopSettings};
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
    fn currencies_for_update<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, (Config, i64)> {
        Box::pin(async move {
            let (settings, revision) = veyndo_shop::store::settings::for_update(tx, tenant).await?;
            Ok((settings.currencies, revision))
        })
    }
}

impl Carts for Server {
    fn currency<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, String> {
        Box::pin(async move { Ok(load_cart(&self.0, request).await?.data.currency) })
    }
}

impl Catalogue for Server {
    fn attribute<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        actor: Option<&'a str>,
        reason: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let mut request = RequestContext::new();
            if let Some(actor) = actor {
                request.insert(
                    "x-rac-user",
                    actor.parse().map_err(|_| bad("Invalid saved actor"))?,
                );
            }
            veyndo_core::history_context::context(tx, &request, reason).await
        })
    }
    fn lock_products<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        cursor: &'a str,
        ceiling: &'a str,
        snapshot: &'a str,
    ) -> PortFuture<'a, Vec<Product>> {
        Box::pin(async move {
            veyndo_catalog::store::products::lock_range(tx, tenant, cursor, ceiling, snapshot).await
        })
    }
    fn store_fixed_prices<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        product: &'a str,
        currency: &'a str,
        prices: Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            veyndo_catalog::store::products::store_fixed_prices(
                tx, tenant, product, currency, prices,
            )
            .await
        })
    }
}

impl axum::extract::FromRef<App> for Pricing {
    fn from_ref(app: &App) -> Self {
        let server = Arc::new(Server(app.clone()));
        Pricing {
            core: app.core.clone(),
            settings: server.clone(),
            carts: server.clone(),
            catalogue: server,
        }
    }
}
