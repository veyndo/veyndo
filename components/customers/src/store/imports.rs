//! Native customer state that reviewed migration imports replay against and compare with.
use serde_json::Value;
use veyndo_core::{Result, State};

/// The imported fields of a customer.
pub struct Customer {
    pub email: String,
    pub profile: Value,
    pub group: String,
    pub automation: Value,
}

/// A customer's imported fields.
pub async fn customer(a: &State, t: &str, id: &str) -> Result<Option<Customer>> {
    Ok(sqlx::query_as!(
        Customer,
        r#"SELECT email,profile,group_name AS "group",automation FROM customers WHERE tenant=$1 AND id=$2"#,
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}
