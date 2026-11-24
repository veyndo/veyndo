//! Atomic company setup through the same company, unit and member writers; a foreign customer rolls back the whole setup.
use crate::store::organization::{member_tx, save_tx};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::history_context::context;
use veyndo_core::{RequestContext, Result, State, bad, uid};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::conditions;
use veyndo_shop::store::settings::scoped_locked;

/// The first unit of a new company: active, in the shop's default currency, without budget or restrictions.
pub fn first_unit(unit: &str, company: &str, name: &Value, currency: &str) -> Value {
    json!({"id":unit,"companyId":company,"parentId":null,"revision":0,"data":{"name":name,"active":true,"currency":currency,"costCenter":"","approvalAboveMinor":null,"budgetMinor":null,"budgetPeriod":"month","restrictedCatalog":false,"requirePurchaseOrder":false,"allowedShipping":[],"allowedPayments":[]}})
}

/// A new company: its ID, a zero revision, its fields and an optional first buyer.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CompanyOnboarding {
    #[serde(deserialize_with = "lenient")]
    customer_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    data: Value,
}

/// Creates a company with its first unit and, when named, links an existing customer as buying member.
pub async fn create(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: CompanyOnboarding = view(v);
    let t = merchant(h)?;
    permit(h, "b2b.manage")?;
    permit(h, "customers.pii")?;
    let customer = input.customer_id.as_deref().filter(|s| !s.is_empty());
    if customer.is_some() {
        permit(h, "customers.read")?;
        permit(h, "customers.write")?;
    }
    let id = input.id.as_deref().ok_or(bad("Company ID required"))?;
    if input.revision != Some(0) {
        return Err(bad("New company revision must be zero"));
    }
    let mut tx = a.db.begin().await?;
    context(&mut tx, h, "b2b.onboarding").await?;
    let company = save_tx(&mut tx, &t, "company.save", v).await?;
    let (settings, _) = scoped_locked(&mut tx, &t, "default", &conditions).await?;
    let unit = format!("b{}", &uid()[..31]);
    let first = first_unit(
        &unit,
        id,
        input.data.get("name").unwrap_or(&Value::Null),
        &settings.currencies.default_currency,
    );
    save_tx(&mut tx, &t, "unit.save", &first).await?;
    if let Some(customer) = customer {
        member_tx(&mut tx,&t,&json!({"unitId":unit,"customerId":customer,"revision":0,"data":{"active":true,"permissions":["buy","lists"],"approvalAboveMinor":null}})).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"id":id,"revision":company["revision"],"unitId":unit,"customerLinked":customer.is_some()}),
    )
}

#[cfg(test)]
mod tests {
    use super::first_unit;
    use serde_json::json;

    #[test]
    fn the_first_unit_starts_open_in_the_default_currency() {
        let unit = first_unit("b1", "acme", &json!({"en-GB":"Acme"}), "EUR");
        assert_eq!(unit["companyId"], "acme");
        assert_eq!(unit["revision"], 0);
        assert_eq!(unit["data"]["currency"], "EUR");
        assert_eq!(unit["data"]["active"], true);
        assert!(unit["data"]["budgetMinor"].is_null());
        assert_eq!(unit["data"]["restrictedCatalog"], false);
    }
}
