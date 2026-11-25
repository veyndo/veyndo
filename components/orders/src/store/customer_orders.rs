//! Orders of a buyer for the customers component: the account's totals and the newest guest order of an email.
use serde_json::Value;
use veyndo_core::{Result, State};
use veyndo_customers::store::customers::OrderTotals;

/// Orders bound to the account's stable ID; a guest order with the same email does not count.
pub async fn account_totals(a: &State, t: &str, customer: Option<&str>) -> Result<OrderTotals> {
    Ok(sqlx::query_as!(
        OrderTotals,
        r#"SELECT count(*) AS "count!",coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric),0)::double precision AS "amount!",max(created_at)::text AS last_order FROM orders WHERE tenant=$1 AND data->'orderCustomer'->>'customerId'=$2"#,
        t,
        customer
    )
    .fetch_one(&a.db)
    .await?)
}

/// The newest guest order of an email: its snapshot and creation time.
pub async fn latest_guest(a: &State, t: &str, email: &str) -> Result<Option<(Value, String)>> {
    Ok(sqlx::query!(
        r#"SELECT data,created_at::text AS "created!" FROM orders WHERE tenant=$1 AND data->'orderCustomer'->>'email'=$2 AND data->'orderCustomer'->>'guest'='true' ORDER BY created_at DESC,id DESC LIMIT 1"#,
        t,
        email
    )
    .fetch_optional(&a.db)
    .await?
    .map(|r| (r.data, r.created)))
}
