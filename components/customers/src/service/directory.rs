//! The merchant's view of customers: standard read fields, the bounded lookup and guest contacts.
use crate::domain::{profile, search};
use crate::store::customers;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Error, Result, State, StatusCode};

/// Adds the standard read fields and order metrics to a customer value.
pub async fn decorate(a: &State, t: &str, email: &str, v: &mut Value) -> Result<()> {
    profile::project_contacts(v);
    let account: Account = view(v);
    let usage = customers::usage(a, t, email).await?;
    merge(
        v,
        json!({"salesChannelId":usage.sales_channel_id,"languageId":usage.language_id,"firstLogin":usage.first_login,"lastLogin":usage.last_login,"lastPaymentMethodId":usage.last_payment_method_id,"groupId":account.customer_group,"guest":false}),
    )?;
    let totals = customers::order_totals(a, t, account.id.as_deref()).await?;
    merge(
        v,
        json!({"orderCount":totals.count,"orderTotalAmount":totals.amount,"lastOrderDate":totals.last_order}),
    )
}

/// The identity and price group of a customer value.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Account {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    customer_group: Value,
}

/// One page of accounts for identity bindings: tenant-scoped, indexed and bounded; no guest-order union.
pub async fn lookup(a: &State, t: &str, after: &str, query: &str, limit: i64) -> Result<Value> {
    let rows = customers::search(a, t, after, &search::literal(query), limit + 1).await?;
    let more = rows.len() > limit as usize;
    let page = &rows[..rows.len().min(limit as usize)];
    let elements: Vec<_> = page
        .iter()
        .map(|r| json!({"id":r.id,"email":r.email,"customerNumber":r.customer_number,"profile":{"name":r.name,"firstName":r.first,"lastName":r.last}}))
        .collect();
    Ok(
        json!({"elements":elements,"hasMore":more,"nextCursor":if more {page.last().map(|r|r.id.clone())} else {None}}),
    )
}

/// A guest as the merchant sees them: the newest order snapshot, never an account.
pub async fn guest(a: &State, t: &str, email: &str) -> Result<Value> {
    let (data, created) = customers::latest_guest_order(a, t, email)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Customer not found".into()))?;
    let order: GuestOrder = view(&data);
    let company = view::<BillingCompany>(&order.billing_address).company;
    let mut profile = order
        .billing_address
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(contact) = order.order_customer.as_object() {
        profile.extend(contact.clone());
    }
    Ok(
        json!({"id":null,"email":email,"customerNumber":null,"guest":true,
        "profile":profile,"company":company,
        "customerGroup":order.customer_group,"active":false,"revision":null,
        "createdAt":created,
        "billingAddress":order.billing_address,"shippingAddress":order.shipping_address}),
    )
}

/// The parts of an order snapshot that describe its guest.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GuestOrder {
    billing_address: Value,
    shipping_address: Value,
    order_customer: Value,
    customer_group: Value,
}

/// The company of a billing address.
#[derive(Deserialize, Default)]
#[serde(default)]
struct BillingCompany {
    company: Value,
}
