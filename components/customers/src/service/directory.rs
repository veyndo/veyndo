//! The merchant's view of customers: standard read fields, the bounded lookup and guest contacts.
use crate::domain::{profile, search};
use crate::store::customers;
use serde_json::{Value, json};
use veyndo_core::{Error, Result, State, StatusCode};

/// Adds the standard read fields and order metrics to a customer value.
pub async fn decorate(a: &State, t: &str, email: &str, v: &mut Value) -> Result<()> {
    profile::project_contacts(v);
    let usage = customers::usage(a, t, email).await?;
    v["salesChannelId"] = json!(usage.sales_channel_id);
    v["languageId"] = json!(usage.language_id);
    v["firstLogin"] = json!(usage.first_login);
    v["lastLogin"] = json!(usage.last_login);
    v["lastPaymentMethodId"] = json!(usage.last_payment_method_id);
    v["groupId"] = v["customerGroup"].clone();
    v["guest"] = json!(false);
    let totals = customers::order_totals(a, t, v["id"].as_str()).await?;
    v["orderCount"] = json!(totals.count);
    v["orderTotalAmount"] = json!(totals.amount);
    v["lastOrderDate"] = json!(totals.last_order);
    Ok(())
}

/// One page of accounts for identity bindings: tenant-scoped, indexed and bounded; no guest-order union.
pub async fn lookup(a: &State, t: &str, after: &str, query: &str, limit: i64) -> Result<Value> {
    let rows = customers::search(a, t, after, &search::literal(query), limit + 1).await?;
    let more = rows.len() > limit as usize;
    let elements: Vec<_> = rows
        .iter()
        .take(limit as usize)
        .map(|r| json!({"id":r.id,"email":r.email,"customerNumber":r.customer_number,"profile":{"name":r.name,"firstName":r.first,"lastName":r.last}}))
        .collect();
    Ok(
        json!({"elements":elements,"hasMore":more,"nextCursor":if more {elements.last().map(|e|e["id"].clone())} else {None}}),
    )
}

/// A guest as the merchant sees them: the newest order snapshot, never an account.
pub async fn guest(a: &State, t: &str, email: &str) -> Result<Value> {
    let (data, created) = customers::latest_guest_order(a, t, email)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Customer not found".into()))?;
    let mut profile = data["billingAddress"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(contact) = data["orderCustomer"].as_object() {
        profile.extend(contact.clone());
    }
    Ok(
        json!({"id":null,"email":email,"customerNumber":null,"guest":true,
        "profile":profile,"company":data["billingAddress"]["company"],
        "customerGroup":data["customerGroup"],"active":false,"revision":null,
        "createdAt":created,
        "billingAddress":data["billingAddress"],"shippingAddress":data["shippingAddress"]}),
    )
}
