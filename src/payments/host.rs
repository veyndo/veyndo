//! The server side of the payment ledger port: adapter configuration and the POS sale completion.
use super::Attempt;
use crate::{App, Result, Value};
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

/// Provider transport and configured worker scope of this process.
pub(crate) struct Gateway<'s>(pub(crate) &'s App);
impl veyndo_payments::ports::Gateway for Gateway<'_> {
    fn call<'a>(
        &'a self,
        tenant: &'a str,
        provider: &'a str,
        version: &'a str,
        environment: &'a str,
        path: &'a str,
        body: &'a Value,
    ) -> PortFuture<'a, Value> {
        Box::pin(super::remote::call(
            self.0,
            tenant,
            provider,
            version,
            environment,
            path,
            body,
        ))
    }
    fn dispatch<'a>(
        &'a self,
        attempt: &'a Attempt,
        operation: &'a str,
        key: &'a str,
        input: &'a Value,
    ) -> PortFuture<'a, Value> {
        Box::pin(super::dispatch(self.0, attempt, operation, key, input))
    }
    fn persist<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        attempt: &'a Attempt,
        operation: &'a str,
        input: &'a Value,
        evidence: &'a Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(super::storage::persist(
            tx, attempt, operation, input, evidence,
        ))
    }
    fn scope(&self) -> (Vec<String>, Vec<String>) {
        let config = crate::runtime_config::get();
        let tenants = config
            .paypal_accounts
            .as_object()
            .map(|accounts| {
                accounts
                    .keys()
                    .filter(|tenant| super::account(tenant).is_ok())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let providers = config
            .payment_services
            .as_object()
            .map(|services| services.keys().cloned().collect())
            .unwrap_or_default();
        (tenants, providers)
    }
    fn cancellable(&self, attempt: &Attempt, evidence: &Value) -> Result<()> {
        super::paypal::validate_order(attempt, evidence)
    }
    fn package<'a>(
        &'a self,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, crate::apps::Manifest> {
        Box::pin(crate::apps::package(self.0, tenant, id, true))
    }
    fn builtin(&self, tenant: &str) -> Value {
        serde_json::json!({"id":"paypal","environment":super::environment(),"configured":super::account(tenant).is_ok(),"apiContract":"PayPal Orders v2"})
    }
}
