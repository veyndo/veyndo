//! The server's implementation of what the inventory component reads from environments and the product index.
use crate::{App, knowledge};
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::PortFuture;
use veyndo_inventory::ports::{Environments, Inventory, ProductIndex};
use veyndo_pricing::catalog::Product;

struct Server;

impl Environments for Server {
    fn is_private_sandbox<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, bool> {
        Box::pin(async move {
            Ok(sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NULL)",
            )
            .bind(tenant)
            .fetch_one(tx)
            .await?)
        })
    }
}

impl ProductIndex for Server {
    fn product_changed<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        product: &'a Product,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            knowledge::sync_product(tx, tenant, &serde_json::json!(product)).await?;
            Ok(())
        })
    }
}

impl axum::extract::FromRef<App> for Inventory {
    fn from_ref(app: &App) -> Self {
        let server = Arc::new(Server);
        Inventory {
            core: app.core.clone(),
            environments: server.clone(),
            index: server,
        }
    }
}
