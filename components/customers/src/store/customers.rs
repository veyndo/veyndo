//! Customer rows as the merchant and the account see them, with order metrics from authoritative records. Columns checked at compile time.
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use sqlx::postgres::PgRow;
use veyndo_core::{Result, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Customer {
    pub id: String,
    pub customer_number: String,
    pub email: String,
    pub profile: Value,
    pub company: Option<String>,
    pub customer_group: String,
    pub active: bool,
    pub revision: i64,
    pub default_billing_address_id: Option<String>,
    pub default_shipping_address_id: Option<String>,
    pub created_at: String,
}

/// Reads a complete `customers` row that also selects `created_at::text AS created` (the merchant directory's dynamic query).
pub fn customer(r: &PgRow) -> Customer {
    Customer {
        id: r.get("id"),
        customer_number: r.get("customer_number"),
        email: r.get("email"),
        profile: r.get("profile"),
        company: r.get("company"),
        customer_group: r.get("group_name"),
        active: r.get("active"),
        revision: r.get("revision"),
        default_billing_address_id: r.get("default_billing_address_id"),
        default_shipping_address_id: r.get("default_shipping_address_id"),
        created_at: r.get("created"),
    }
}

/// The account of an email as the customer reads it.
pub async fn by_email(a: &State, t: &str, email: &str) -> Result<Customer> {
    Ok(sqlx::query_as!(
        Customer,
        r#"SELECT id,customer_number,email,profile,company,group_name AS customer_group,active,revision,default_billing_address_id,default_shipping_address_id,created_at::text AS "created_at!" FROM customers WHERE tenant=$1 AND email=$2"#,
        t,
        email
    )
    .fetch_one(&a.db)
    .await?)
}

/// Where and when the account was used.
pub struct Usage {
    pub sales_channel_id: String,
    pub language_id: String,
    pub first_login: Option<String>,
    pub last_login: Option<String>,
    pub last_payment_method_id: Option<String>,
}

pub async fn usage(a: &State, t: &str, email: &str) -> Result<Usage> {
    Ok(sqlx::query_as!(
        Usage,
        "SELECT sales_channel_id,language_id,first_login::text AS first_login,last_login::text AS last_login,last_payment_method_id FROM customers WHERE tenant=$1 AND email=$2",
        t,
        email
    )
    .fetch_one(&a.db)
    .await?)
}

pub struct OrderTotals {
    pub count: i64,
    pub amount: f64,
    pub last_order: Option<String>,
}

/// Orders bound to the account's stable ID; a guest order with the same email does not count.
pub async fn order_totals(a: &State, t: &str, customer: Option<&str>) -> Result<OrderTotals> {
    Ok(sqlx::query_as!(
        OrderTotals,
        r#"SELECT count(*) AS "count!",coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric),0)::double precision AS "amount!",max(created_at)::text AS last_order FROM orders WHERE tenant=$1 AND data->'orderCustomer'->>'customerId'=$2"#,
        t,
        customer
    )
    .fetch_one(&a.db)
    .await?)
}

pub struct Found {
    pub id: String,
    pub email: String,
    pub customer_number: Option<String>,
    pub name: Option<String>,
    pub first: Option<String>,
    pub last: Option<String>,
}

/// Accounts after the cursor whose email, names or number contain the escaped, lower-cased text.
pub async fn search(
    a: &State,
    t: &str,
    after: &str,
    literal: &str,
    limit: i64,
) -> Result<Vec<Found>> {
    Ok(sqlx::query_as!(
        Found,
        r#"SELECT id,email,customer_number AS "customer_number?",profile->>'name' AS name,profile->>'firstName' AS first,profile->>'lastName' AS last FROM customers WHERE tenant=$1 AND id>$2 AND ($3='' OR lower(email||' '||coalesce(profile->>'name','')||' '||coalesce(profile->>'firstName','')||' '||coalesce(profile->>'lastName','')||' '||coalesce(customer_number,'')) LIKE '%'||$3||'%') ORDER BY id LIMIT $4"#,
        t,
        after,
        literal,
        limit
    )
    .fetch_all(&a.db)
    .await?)
}

/// The newest guest order of an email: its snapshot and creation time.
pub async fn latest_guest_order(
    a: &State,
    t: &str,
    email: &str,
) -> Result<Option<(Value, String)>> {
    Ok(sqlx::query!(
        r#"SELECT data,created_at::text AS "created!" FROM orders WHERE tenant=$1 AND data->'orderCustomer'->>'email'=$2 AND data->'orderCustomer'->>'guest'='true' ORDER BY created_at DESC,id DESC LIMIT 1"#,
        t,
        email
    )
    .fetch_optional(&a.db)
    .await?
    .map(|r| (r.data, r.created)))
}

/// Remembers the payment method an active customer used last.
pub async fn remember_payment(a: &State, t: &str, customer: &str, method: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE customers SET last_payment_method_id=$1 WHERE tenant=$2 AND id=$3 AND active AND last_payment_method_id IS DISTINCT FROM $1",
        method,
        t,
        customer
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// The ID of an active customer account by its email.
pub async fn active_id(a: &State, t: &str, email: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM customers WHERE tenant=$1 AND email=$2 AND active",
        t,
        email
    )
    .fetch_one(&a.db)
    .await?)
}

/// Fills the empty profile of the synthetic demo buyer of a newly provisioned shop.
pub async fn seed_demo_profile(
    conn: &mut sqlx::PgConnection,
    t: &str,
    profile: &Value,
) -> Result<()> {
    sqlx::query!("UPDATE customers SET profile=$1 WHERE tenant=$2 AND email='buyer@example.test' AND profile='{}'::jsonb", profile, t).execute(conn).await?;
    Ok(())
}

/// The email of a customer by ID.
pub async fn email_of(a: &State, t: &str, id: Option<&str>) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT email FROM customers WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}
