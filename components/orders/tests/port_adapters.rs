//! Test adapter for the orders port `Payments`, so the orders mirror builds and tests without the server.
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::PortFuture;
use veyndo_orders::ports::Payments;

/// Prints transfer instructions for orders paid by bank transfer, nothing for other payments.
struct TransferOnly;

impl TransferOnly {
    fn instructions(order: &Value) -> Option<Value> {
        (order["paymentMethod"] == "transfer")
            .then(|| json!({"recipient": "Test GmbH", "reference": order["id"]}))
    }
}

impl Payments for TransferOnly {
    fn document_instructions<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
        order: &'a Value,
    ) -> PortFuture<'a, Option<Value>> {
        let answer = Self::instructions(order);
        Box::pin(async move { Ok(answer) })
    }
}

#[test]
fn payments_port_is_satisfiable_without_the_server() {
    let _port: Arc<dyn Payments> = Arc::new(TransferOnly);
    let transfer = TransferOnly::instructions(&json!({"id": "o-1", "paymentMethod": "transfer"}));
    assert_eq!(transfer.unwrap()["reference"], "o-1");
    assert!(TransferOnly::instructions(&json!({"id": "o-2", "paymentMethod": "card"})).is_none());
}
