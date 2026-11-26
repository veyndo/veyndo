//! The server's implementation of what the catalog component reads from carts, orders, environments and the history.
use crate::cart_storage::load_cart;
use crate::foundation::App;
use crate::localization::language_context;
use crate::{commerce, marketing, performance};
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_catalog::ports::{
    Catalog, ContentLanguages, ImageProvider, Images, Shop, Shopper, Shoppers, Visit,
};
use veyndo_core::PortFuture;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;

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
            veyndo_orders::store::order_reads::bought(
                &self.0,
                &shopper.tenant,
                (&shopper.cart, shopper.customer.as_deref()),
                product,
            )
            .await
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
            let settings = veyndo_shop::store::settings::shared(tx, tenant).await?;
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
        Box::pin(async move {
            Ok(veyndo_core::store::environments::parent(&self.0, tenant)
                .await?
                .is_some())
        })
    }
    fn attribute<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        request: &'a RequestContext,
        reason: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_core::store::history_context::context(
            tx, request, reason,
        ))
    }
}

impl Images for Server {
    fn provider<'a>(&'a self) -> PortFuture<'a, ImageProvider> {
        Box::pin(async move {
            Ok(ImageProvider {
                configured: veyndo_cognition::images::configured(&self.0.models.inference).await,
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
            veyndo_cognition::images::create(&self.0.models.inference, prompt, source).await
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
