//! What the inventory component needs from other components. The server implements these; tests use their own.
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::{PortFuture, State};
use veyndo_pricing::catalog::Product;

/// The environments of a shop, as far as stock writes ask about them.
pub trait Environments: Send + Sync {
    /// Whether the shop is a private sandbox: an environment nobody else previews.
    fn is_private_sandbox<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, bool>;
}

/// The search and knowledge index of products.
pub trait ProductIndex: Send + Sync {
    /// Brings the index in line with the changed product, inside the transaction of the change.
    fn product_changed<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        product: &'a Product,
    ) -> PortFuture<'a, ()>;
}

/// The state inventory operations take when they need another component.
#[derive(Clone)]
pub struct Inventory {
    pub core: State,
    pub environments: Arc<dyn Environments>,
    pub index: Arc<dyn ProductIndex>,
}
