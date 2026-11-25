//! Customer rule authority is loaded by tenant and stable customer ID, with aggregate history and calendar age.
use serde::Deserialize;
use serde_json::{Value, json};
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
    let row=sqlx::query!("SELECT active,email,group_name,customer_number,profile,automation,company,extract(year FROM age(current_date,(profile->>'birthday')::date))::double precision AS age,abs(current_date-first_login::date)::double precision AS first_days,abs(current_date-last_login::date)::double precision AS last_days FROM customers WHERE tenant=$1 AND id=$2", &c.tenant, c.data.customer_id.as_deref()).fetch_optional(&mut *conn).await?;
    let Some(r) = row else {
        return Ok(
            json!({"loggedIn":false,"guest":true,"email":null,"active":false,"group":c.data.group,"tags":[],"salutationId":null,"lastName":null,"customerNumber":null,"affiliateCode":null,"campaignCode":null,"requestedGroupId":null,"age":null,"orderCount":null,"orderTotalAmount":null,"reviewCount":null,"daysSinceFirstLogin":null,"daysSinceLastLogin":null,"daysSinceLastOrder":null,"isCompany":false,"newsletter":false,"createdByAdmin":false,"differentAddresses":false}),
        );
    };
    let profile: Value = r.profile.clone();
    let a: Value = r.automation.clone();
    let stats=sqlx::query!(r#"SELECT count(*) AS "count!",coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric),0)::double precision AS "amount!",abs(current_date-max(created_at)::date)::double precision AS days FROM orders WHERE tenant=$1 AND data->'orderCustomer'->>'customerId'=$2"#, &c.tenant, c.data.customer_id.as_deref()).fetch_one(&mut *conn).await?;
    let reviews: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "reviews!" FROM product_reviews WHERE tenant=$1 AND session='customer:'||$2"#, &c.tenant, c.data.customer_id.as_deref())
    .fetch_one(&mut *conn)
    .await?;
    Ok(
        json!({"loggedIn":true,"guest":false,"active":r.active.clone(),"email":r.email.clone(),"group":r.group_name.clone(),"customerNumber":r.customer_number.clone(),"salutationId":profile["salutationId"],"lastName":profile["lastName"],"age":r.age.clone(),"birthday":profile["birthday"],"tags":a["tags"].as_array().cloned().unwrap_or_default(),"customFields":a["customFields"].as_object().cloned().unwrap_or_default(),"affiliateCode":a["affiliateCode"],"campaignCode":a["campaignCode"],"requestedGroupId":a["requestedGroupId"],"newsletter":a["newsletter"].as_bool().unwrap_or(false),"createdByAdmin":a["createdByAdmin"].as_bool().unwrap_or(false),"isCompany":r.company.clone().is_some_and(|s|!s.is_empty()),"differentAddresses":input.billing!=input.shipping,"orderCount":stats.count.clone(),"orderTotalAmount":stats.amount.clone(),"reviewCount":reviews,"daysSinceFirstLogin":r.first_days.clone(),"daysSinceLastLogin":r.last_days.clone(),"daysSinceLastOrder":stats.days.clone()}),
    )
}
