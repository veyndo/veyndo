//! What the orders component needs from the server.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::PortFuture;

pub trait Payments: Send + Sync {
    /// The recipient instructions an invoice prints for an order paid by transfer; `None` for every other payment.
    fn document_instructions<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        order: &'a Value,
    ) -> PortFuture<'a, Option<Value>>;
}
