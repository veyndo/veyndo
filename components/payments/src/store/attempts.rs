//! Row mapping of the payment attempt ledger.
use crate::domain::attempt::Attempt;
use sqlx::Row;

pub fn attempt(r: &sqlx::postgres::PgRow) -> Attempt {
    Attempt {
        id: r.get("id"),
        provider: r.get("provider"),
        context: r.get("provider_context"),
        tenant: r.get("tenant"),
        order: r.get("order_id"),
        amount: r.get("amount_minor"),
        currency: r.get("currency"),
        currency_scale: r.get("currency_scale"),
        state: r.get("state"),
        provider_order: r.get("provider_order"),
        capture: r.get("capture_id"),
        refunded: r.get("refunded_minor"),
        adapter_version: r.get("adapter_version"),
        environment: r.get("environment"),
        bn_code: r.get("bn_code"),
    }
}
