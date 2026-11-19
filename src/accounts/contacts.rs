//! Native customer projection; shared contact syntax never grants identity or PII access.
use crate::*;
pub(crate) use veyndo_pricing::customer::Contact;
pub(crate) fn customer_value(r: &sqlx::postgres::PgRow) -> Value {
    json!(veyndo_customers::store::customers::customer(r))
}
