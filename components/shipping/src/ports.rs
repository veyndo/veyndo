//! What the shipping component needs from components above it: where orders, carts, overrides, rules, flows and purchasing units refer to a method. The server implements it.
use sqlx::PgConnection;
use veyndo_core::PortFuture;

/// One method looked up in the documents that may refer to it.
pub struct MethodReference<'a> {
    pub tenant: &'a str,
    /// The method ID.
    pub id: &'a str,
    /// Only carts and orders of this sales channel; `None` counts all.
    pub channel: Option<&'a str>,
    /// `shipping` or `payments`, the first path segment of a settings override.
    pub area: &'a str,
    /// The path of the method ID inside an order snapshot.
    pub snapshot: &'a [&'a str],
    /// The checkout key a cart stores the selection under.
    pub selection: &'a str,
    /// The older checkout key of the same selection.
    pub legacy: &'a str,
    /// The purchasing unit key that lists allowed methods.
    pub allowed: &'a str,
}

/// Counts of documents that refer to a method, read inside the caller's transaction.
pub trait MethodReferences: Send + Sync {
    /// Orders whose snapshot or checkout selection names the method.
    fn orders<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64>;
    /// Open carts that selected the method.
    fn carts<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64>;
    /// Channel settings overrides of the method.
    fn overrides<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64>;
    /// Rules and flows that mention the method ID.
    fn rules<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64>;
    /// Purchasing units that allow the method.
    fn units<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64>;
}
