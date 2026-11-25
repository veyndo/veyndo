//! Storage of the external order import through its owners: replay by idempotency key (orders), the locked product lines and their labels (catalog), the share-locked channel (marketing) and the imported cart and order rows (checkout).
pub use veyndo_catalog::store::imported_lines::{
    Label, LineProduct, hidden_in_channel, labels, lock_products, parents,
};
pub use veyndo_checkout::store::app_carts::insert_completed as insert_cart;
pub use veyndo_checkout::store::purchase::NewOrder;
pub use veyndo_marketing::store::app_channels::lock_document as lock_channel;
pub use veyndo_orders::store::app_orders::{Imported, imported};

/// Stores an imported order; the unique idempotency key rejects a concurrent duplicate.
pub async fn insert_order(
    conn: &mut sqlx::PgConnection,
    o: NewOrder<'_>,
) -> veyndo_core::Result<()> {
    veyndo_checkout::store::purchase::insert_order(conn, &o).await
}
