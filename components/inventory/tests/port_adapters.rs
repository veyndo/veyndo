//! Test adapters for the inventory ports (`Environments`, `ProductIndex`), so the inventory mirror builds and tests without the server.
use sqlx::PgConnection;
use std::sync::{Arc, Mutex};
use veyndo_core::PortFuture;
use veyndo_inventory::ports::{Environments, ProductIndex};
use veyndo_pricing::catalog::Product;

/// Every shop is live; no shop is a private sandbox.
struct LiveShops;

impl Environments for LiveShops {
    fn is_private_sandbox<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
}

/// Records which products the index was told about.
#[derive(Default)]
struct RecordedIndex(Mutex<Vec<String>>);

impl ProductIndex for RecordedIndex {
    fn product_changed<'a>(
        &'a self,
        _: &'a mut PgConnection,
        tenant: &'a str,
        product: &'a Product,
    ) -> PortFuture<'a, ()> {
        self.0
            .lock()
            .unwrap()
            .push(format!("{tenant}/{}", product.id));
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn inventory_ports_are_satisfiable_without_the_server() {
    let environments: Arc<dyn Environments> = Arc::new(LiveShops);
    let index = Arc::new(RecordedIndex::default());
    let _index: Arc<dyn ProductIndex> = index.clone();
    drop(environments);
    assert!(index.0.lock().unwrap().is_empty());
}
