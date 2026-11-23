//! The server's implementation of what the catalog component reads from carts, orders, environments and the history.
use crate::{
    App, RequestContext, commerce, history, language_context, load_cart, marketing, performance,
    staging, tenant,
};
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_catalog::ports::{
    Catalog, ContentLanguages, ImageProvider, Images, Shop, Shopper, Shoppers, Visit,
};
use veyndo_core::PortFuture;

struct Server(App);

impl Shoppers for Server {
    fn current<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, Shopper> {
        Box::pin(async move {
            let cart = load_cart(&self.0, request).await?;
            Ok(Shopper {
                tenant: cart.tenant,
                cart: cart.id,
                customer: cart.data.customer_id,
            })
        })
    }
    fn bought<'a>(&'a self, shopper: &'a Shopper, product: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async move {
            Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders o WHERE o.tenant=$1 AND (o.cart_id=$2 OR ($3::text IS NOT NULL AND o.data->'orderCustomer'->>'customerId'=$3)) AND EXISTS(SELECT 1 FROM jsonb_array_elements(o.data->'cart'->'lineItems') i WHERE i->>'id'=$4))")
                .bind(&shopper.tenant)
                .bind(&shopper.cart)
                .bind(&shopper.customer)
                .bind(product)
                .fetch_one(&self.0.db)
                .await?)
        })
    }
}

impl Shop for Server {
    fn languages<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, ContentLanguages> {
        Box::pin(async move {
            let (settings, _) = commerce::config(&self.0, tenant).await?;
            Ok(ContentLanguages {
                main: settings.main_locale.clone(),
                enabled: settings.locales.clone(),
            })
        })
    }
    fn languages_shared<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, ContentLanguages> {
        Box::pin(async move {
            let data: serde_json::Value =
                sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1 FOR SHARE")
                    .bind(tenant)
                    .fetch_one(tx)
                    .await?;
            let settings = commerce::decode_config(data)?;
            Ok(ContentLanguages {
                main: settings.main_locale,
                enabled: settings.locales,
            })
        })
    }
    fn locales<'a>(&'a self, ids: &'a [String]) -> PortFuture<'a, Vec<String>> {
        Box::pin(async move {
            let languages = performance::languages(&self.0).await?;
            Ok(ids
                .iter()
                .filter_map(|id| {
                    languages
                        .iter()
                        .find(|l| l.id == *id)
                        .map(|l| l.locale.clone())
                })
                .collect())
        })
    }
    fn visit<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, Visit> {
        Box::pin(async move {
            let tenant = tenant(request)?;
            let (locale, _) = language_context(&self.0, request).await?;
            let channel =
                marketing::channel(&self.0, request, marketing::channel_id(request), &locale)
                    .await?;
            Ok(Visit {
                tenant,
                locale,
                navigation_root: channel
                    .and_then(|c| c.navigation_category_id)
                    .unwrap_or_else(|| "catalog-root".into()),
            })
        })
    }
    fn admit_product<'a>(
        &'a self,
        request: &'a RequestContext,
        product: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(marketing::admit_product(&self.0, request, product))
    }
    fn is_preview<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async move { Ok(staging::parent(&self.0, tenant).await?.is_some()) })
    }
    fn attribute<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        request: &'a RequestContext,
        reason: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(history::context(tx, request, reason))
    }
}

impl Images for Server {
    fn provider<'a>(&'a self) -> PortFuture<'a, ImageProvider> {
        Box::pin(async move {
            Ok(ImageProvider {
                configured: veyndo_cognition::images::configured(&self.0.inference).await,
                model: veyndo_cognition::images::model(),
            })
        })
    }
    fn create<'a>(
        &'a self,
        tenant: &'a str,
        prompt: &'a str,
        source: Option<(String, Vec<u8>)>,
    ) -> PortFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let _cluster =
                performance::cluster_lease::Lease::acquire(&self.0, tenant, "model", 2).await?;
            veyndo_cognition::images::create(&self.0.inference, prompt, source).await
        })
    }
}

impl axum::extract::FromRef<App> for Catalog {
    fn from_ref(app: &App) -> Self {
        let server = Arc::new(Server(app.clone()));
        Catalog {
            core: app.core.clone(),
            shoppers: server.clone(),
            shop: server.clone(),
            images: server,
        }
    }
}
