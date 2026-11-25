//! The server's implementation of the catalog registries port: languages from the shop store, sales channels from the marketing store.
use crate::foundation::App;
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_catalog::ports::Registries;
use veyndo_core::PortFuture;

struct Server(veyndo_core::State);

impl Registries for Server {
    fn language_id<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        locale: &'a str,
    ) -> PortFuture<'a, Option<String>> {
        Box::pin(veyndo_shop::store::languages::id_of(conn, locale))
    }
    fn lock_channels<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(veyndo_marketing::store::channel_ids::lock_ids(tx, tenant))
    }
    fn channels<'a>(&'a self, tenant: &'a str, limit: i64) -> PortFuture<'a, Vec<(String, Value)>> {
        Box::pin(veyndo_marketing::store::channel_ids::first(
            &self.0, tenant, limit,
        ))
    }
}

/// Registers the registries port with the process state.
pub(super) fn register(a: &App) {
    veyndo_catalog::ports::register_registries(Arc::new(Server(a.core.clone())));
}
