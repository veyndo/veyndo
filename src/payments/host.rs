//! The server side of the payment ledger port: adapter configuration and the POS sale completion.
use crate::{Result, Value};
use veyndo_core::PortFuture;

pub(crate) struct PaymentHost;
impl veyndo_payments::ports::Host for PaymentHost {
    fn service(&self, provider: &str, version: &str, environment: &str) -> Result<Value> {
        super::remote::config(provider, version, environment)
    }
    fn captured<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        order: &'a mut Value,
        state: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(crate::pos::payment_captured(tx, tenant, order, state))
    }
}
