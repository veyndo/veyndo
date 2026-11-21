//! The locked SKU snapshot is owned by the checkout component; the language chain is prepared here before the transaction.
use crate::*;
pub(crate) use veyndo_checkout::store::product_snapshot::snapshot;

/// Prepare language metadata before the checkout holds a pool connection.
pub(crate) async fn language(a: &App, c: &StoredCart) -> Result<(Vec<String>, Vec<String>)> {
    veyndo_checkout::service::language::language(&a.core, c).await
}
