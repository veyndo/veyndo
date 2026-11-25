//! Customer rule authority is loaded by tenant and stable customer ID, with aggregate history and calendar age. Accounts and orders come through the records port, reviews from the catalog store, on the caller's connection.
use crate::ports::records;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::store::rule_products;
use veyndo_core::Result;
use veyndo_core::document::view;
use veyndo_shop::types::cart::StoredCart;
/// The JSON fields `customer` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerInput {
    billing: Value,
    shipping: Value,
}

pub async fn customer(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    facts: &Value,
) -> Result<Value> {
    let input: CustomerInput = view(facts);
    let records = records()?;
    let customer_id = c.data.customer_id.as_deref();
    let row = records.rule_customer(conn, &c.tenant, customer_id).await?;
    let Some(r) = row else {
        return Ok(
            json!({"loggedIn":false,"guest":true,"email":null,"active":false,"group":c.data.group,"tags":[],"salutationId":null,"lastName":null,"customerNumber":null,"affiliateCode":null,"campaignCode":null,"requestedGroupId":null,"age":null,"orderCount":null,"orderTotalAmount":null,"reviewCount":null,"daysSinceFirstLogin":null,"daysSinceLastLogin":null,"daysSinceLastOrder":null,"isCompany":false,"newsletter":false,"createdByAdmin":false,"differentAddresses":false}),
        );
    };
    let profile: Profile = view(&r.profile);
    let a: Automation = view(&r.automation);
    let stats = records
        .customer_orders(conn, &c.tenant, customer_id)
        .await?;
    let reviews = rule_products::customer_reviews(conn, &c.tenant, customer_id).await?;
    Ok(
        json!({"loggedIn":true,"guest":false,"active":r.active.clone(),"email":r.email.clone(),"group":r.group_name.clone(),"customerNumber":r.customer_number.clone(),"salutationId":profile.salutation_id,"lastName":profile.last_name,"age":r.age.clone(),"birthday":profile.birthday,"tags":a.tags.as_array().cloned().unwrap_or_default(),"customFields":a.custom_fields.as_object().cloned().unwrap_or_default(),"affiliateCode":a.affiliate_code,"campaignCode":a.campaign_code,"requestedGroupId":a.requested_group_id,"newsletter":a.newsletter.as_bool().unwrap_or(false),"createdByAdmin":a.created_by_admin.as_bool().unwrap_or(false),"isCompany":r.company.clone().is_some_and(|s|!s.is_empty()),"differentAddresses":input.billing!=input.shipping,"orderCount":stats.count.clone(),"orderTotalAmount":stats.amount.clone(),"reviewCount":reviews,"daysSinceFirstLogin":r.first_days.clone(),"daysSinceLastLogin":r.last_days.clone(),"daysSinceLastOrder":stats.days.clone()}),
    )
}

/// The profile fields customer rules read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Profile {
    salutation_id: Value,
    last_name: Value,
    birthday: Value,
}

/// The automation data customer rules read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Automation {
    tags: Value,
    custom_fields: Value,
    affiliate_code: Value,
    campaign_code: Value,
    requested_group_id: Value,
    newsletter: Value,
    created_by_admin: Value,
}
