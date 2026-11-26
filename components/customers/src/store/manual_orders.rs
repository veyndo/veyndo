//! Customers of merchant manual orders and the CRM detail: the active customer a draft is for, the full customer row and the customer's recent orders.
use super::customers::Customer as Account;
use serde_json::{Value, json};
use veyndo_core::document::merge;
use veyndo_core::{Result, State};

/// The cart-relevant identity of an active customer.
pub struct Customer {
    pub id: String,
    pub email: String,
    pub group: String,
    pub company: Option<String>,
}

/// An active customer by ID.
pub async fn active_customer(a: &State, t: &str, id: &str) -> Result<Option<Customer>> {
    Ok(sqlx::query_as!(
        Customer,
        r#"SELECT id,email,group_name AS "group",company FROM customers WHERE tenant=$1 AND id=$2 AND active"#,
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// A customer by email as the merchant directory shows it; `None` for a guest.
pub async fn find_by_email(a: &State, t: &str, email: &str) -> Result<Option<Account>> {
    Ok(sqlx::query_as!(
        Account,
        r#"SELECT id,customer_number,email,profile,company,group_name AS customer_group,active,revision,default_billing_address_id,default_shipping_address_id,created_at::text AS "created_at!" FROM customers WHERE tenant=$1 AND email=$2"#,
        t,
        email
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The customer's 100 most recent orders (without cart tokens), each with `createdAt`.
pub async fn orders(a: &State, t: &str, email: &str) -> Result<Vec<Value>> {
    let rows=sqlx::query!(r#"SELECT o.data #- '{cart,token}' AS "data!",o.created_at::text AS "created!" FROM orders o JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE o.tenant=$1 AND ((o.data->'orderCustomer'->>'customerId')=(SELECT id FROM customers WHERE tenant=o.tenant AND email=$2) OR (o.data->'orderCustomer'->>'guest'='true' AND o.data->'orderCustomer'->>'email'=$2) OR (NOT o.data ? 'orderCustomer' AND c.data->>'email'=$2)) ORDER BY o.created_at DESC LIMIT 100"#, t, email).fetch_all(&a.db).await?;
    rows.into_iter()
        .map(|r| {
            let mut o = r.data;
            merge(&mut o, json!({ "createdAt": r.created }))?;
            Ok(o)
        })
        .collect()
}
