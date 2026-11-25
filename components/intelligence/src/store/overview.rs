//! The bounded shop overview snapshot: order counts, revenue per currency, recent orders and activity, the learning timeline, experiment variants, the semantic index state, proposal counts and channel traffic.
use serde_json::Value;
use veyndo_core::{Result, State};

/// The overview facts document of a shop, as `overview.sql` builds it in one statement.
pub async fn facts(a: &State, tenant: &str) -> Result<Value> {
    Ok(sqlx::query_file_scalar!("src/store/overview.sql", tenant)
        .fetch_one(&a.db)
        .await?)
}
