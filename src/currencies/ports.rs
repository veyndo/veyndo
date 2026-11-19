//! The server's implementation of what the pricing component reads from the shop settings, the cart and the catalogue.
use crate::{App, RequestContext, Row, bad, commerce, history, load_cart, product};
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::Arc;
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
            let row = sqlx::query(
                "SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
            )
            .bind(tenant)
            .fetch_one(tx)
            .await?;
            Ok((
                commerce::decode_config(row.get("data"))?.currencies,
                row.get("revision"),
            ))
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
            history::context(tx, &request, reason).await
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
            let rows = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id>$2 AND id<=$3 AND created_at<=$4::text::timestamptz ORDER BY id LIMIT 100 FOR UPDATE")
                .bind(tenant)
                .bind(cursor)
                .bind(ceiling)
                .bind(snapshot)
                .fetch_all(tx)
                .await?;
            Ok(rows.iter().map(product).collect())
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
            sqlx::query("UPDATE products SET extra=jsonb_set(extra,'{currencyPrices}',coalesce(extra->'currencyPrices','{}') || jsonb_build_object($1::text,$2::jsonb)),revision=revision+1 WHERE tenant=$3 AND id=$4")
                .bind(currency)
                .bind(prices)
                .bind(tenant)
                .bind(product)
                .execute(tx)
                .await?;
            Ok(())
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
