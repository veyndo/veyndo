//! Local flow mutations journal the effect in the same transaction; customer authority changes revoke existing sessions. Orders, customers and carts are changed through the records port, customer groups and download grants by their owners' stores, all in this transaction.
use crate::domain::flow_text::effective;
use crate::ports::{Records, records};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::store::rule_products;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Result, State, bad, uid};
use veyndo_pricing::store::group_automation;
use veyndo_shop::store::settings::scoped_locked;

/// One flow step: who it runs as, the language of its note, its idempotency key and its action.
pub struct Step<'a> {
    pub actor: Option<&'a str>,
    pub locale: &'a str,
    pub key: &'a str,
    pub action: &'a str,
}

/// The automation data a flow action changes: the customer's group, the customer or the order, locked for the change; with the group's id when it is the group's data.
async fn automation_target(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    (records, t): (&dyn Records, &str),
    customer_id: Option<&str>,
    order: &Value,
    (group, customer): (bool, bool),
) -> Result<(Value, Option<String>)> {
    Ok(if group {
        // The customer row stays locked, so its group cannot change before the group's data is stored.
        let row = records.lock_customer_group(tx, t, customer_id).await?;
        (group_automation::lock(tx, t, &row).await?, Some(row))
    } else if customer {
        (
            records
                .lock_customer_automation(tx, t, customer_id)
                .await?
                .ok_or(bad("Flow customer unavailable"))?,
            None,
        )
    } else {
        (order.get("automation").cloned().unwrap_or(json!({})), None)
    })
}

/// The configuration of one local flow action, as the flow saved it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FlowConfig {
    #[serde(deserialize_with = "lenient")]
    group_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    tags: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    field: Option<String>,
    value: Value,
    affiliate_code: Value,
    campaign_code: Value,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    instruction: Value,
}

/// Applies one local flow action to its order or customer once per flow key.
pub async fn execute(
    a: &State,
    t: &str,
    f: Step<'_>,
    config: &Value,
    id: &str,
    conditions: &(dyn Fn(&Value) -> Result<()> + Sync),
) -> Result<Value> {
    let (actor, locale, key, action) = (f.actor, f.locale, f.key, f.action);
    let step: FlowConfig = view(config);
    let records = records()?;
    let mut tx = a.db.begin().await?;
    sqlx::query!("SELECT set_config('veyndo.actor',$1,true) AS actor,set_config('veyndo.source','flow',true) AS source,set_config('veyndo.reason',$2,true) AS reason", actor, format!("flow:{key}:{action}")).fetch_one(&mut *tx).await?;
    let (settings, _) = scoped_locked(&mut tx, t, "default", conditions).await?;
    if action == "action.change.customer.group"
        && !settings
            .customer_groups
            .iter()
            .any(|g| Some(g.id.as_str()) == step.group_id.as_deref())
    {
        return Err(bad("Unknown configured customer group"));
    }
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,33))",
        format!("flow-effect:{t}:{key}")
    )
    .execute(&mut *tx)
    .await?;
    if let Some(v) = records.flow_activity(&mut tx, (t, id), key).await? {
        return Ok(v);
    }
    let mut order = records.lock_order(&mut tx, t, id).await?;
    let customer_id = view::<OrderRefs>(&order)
        .order_customer
        .customer_id
        .as_str()
        .map(String::from);
    let customer = action.contains("customer");
    let group = action == "action.set.customer.group.custom.field";
    let (mut data, group_id) = automation_target(
        &mut tx,
        (records, t),
        customer_id.as_deref(),
        &order,
        (group, customer),
    )
    .await?;
    if action.ends_with(".tag") {
        let mut tags = view::<AutomationFields>(&data)
            .tags
            .as_array()
            .cloned()
            .unwrap_or_default();
        for tag in step.tags.as_ref().ok_or(bad("Flow tags required"))? {
            if action.contains(".remove.") {
                tags.retain(|v| v != tag)
            } else if !tags.contains(tag) {
                tags.push(tag.clone())
            }
        }
        merge(&mut data, json!({ "tags": tags }))?;
    } else if action.contains("custom.field") {
        if !view::<AutomationFields>(&data).custom_fields.is_object() {
            merge(&mut data, json!({ "customFields": {} }))?;
        }
        let field = step.field.as_deref().ok_or(bad("Flow field required"))?;
        data.get_mut("customFields")
            .and_then(Value::as_object_mut)
            .ok_or(bad("Flow custom fields unavailable"))?
            .insert(field.into(), step.value.clone());
    } else if action.ends_with("affiliate.and.campaign.code") {
        let codes = Codes {
            affiliate_code: &step.affiliate_code,
            campaign_code: &step.campaign_code,
        };
        merge(&mut data, &codes)?;
        if !customer {
            merge(&mut order, &codes)?;
        }
    } else if action == "action.grant.download.access" {
        if step.value == false {
            rule_products::revoke_downloads(&mut tx, t, id).await?;
        } else {
            rule_products::grant_downloads(&mut tx, t, id, &order).await?;
        }
    } else if action == "action.change.customer.group" || action == "action.change.customer.status"
    {
        let n = records
            .set_customer_group_and_status(
                &mut tx,
                (step.group_id.as_deref(), step.active),
                t,
                customer_id.as_deref(),
            )
            .await?;
        if n != 1 {
            return Err(bad("Flow customer unavailable"));
        }
        records
            .end_customer_sessions(&mut tx, t, customer_id.as_deref())
            .await?;
        records
            .sign_out_carts(&mut tx, &uid(), t, customer_id.as_deref())
            .await?;
    } else if action != "note" {
        return Err(bad("Unsupported local action"));
    }
    if let Some(group_id) = &group_id {
        group_automation::save(&mut tx, &data, t, group_id).await?;
    } else if customer
        && action != "action.change.customer.group"
        && action != "action.change.customer.status"
    {
        records
            .save_customer_automation(&mut tx, &data, t, customer_id.as_deref())
            .await?;
    } else if !customer && action != "note" && action != "action.grant.download.access" {
        let revision = view::<OrderRefs>(&order).revision.as_i64().unwrap_or(1);
        merge(
            &mut order,
            json!({ "automation": data, "revision": revision + 1 }),
        )?;
        records.save_order(&mut tx, &order, t, id).await?;
    }

    let result = json!({"flowKey":key,"action":action,"orderId":id,"text":effective(&step.instruction, locale, &settings.main_locale),"config":config});
    records
        .add_flow_activity(&mut tx, (t, id), actor, &result)
        .await?;
    tx.commit().await?;
    Ok(result)
}

/// The affiliate and campaign codes a flow action writes onto automation data and the order.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Codes<'a> {
    affiliate_code: &'a Value,
    campaign_code: &'a Value,
}

/// The order fields a flow action reads: its customer and revision.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OrderRefs {
    #[serde(deserialize_with = "lenient")]
    order_customer: CustomerRef,
    revision: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerRef {
    customer_id: Value,
}

/// The tags and custom fields of automation data.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AutomationFields {
    tags: Value,
    custom_fields: Value,
}

#[cfg(test)]
mod tests {
    use super::Codes;
    use serde_json::{Value, json};
    use veyndo_core::document::merge;

    #[test]
    fn flow_codes_and_order_automation_keep_the_indexed_shape() {
        let (affiliate, campaign) = (json!("aff"), json!(null));
        for data in [json!(null), json!({"tags":["a"],"affiliateCode":"old"})] {
            let mut old = data.clone();
            old["affiliateCode"] = affiliate.clone();
            old["campaignCode"] = campaign.clone();
            let mut new = data;
            merge(
                &mut new,
                &Codes {
                    affiliate_code: &affiliate,
                    campaign_code: &campaign,
                },
            )
            .unwrap();
            assert_eq!(new, old);
        }
        let order = json!({"id":"o","revision":3,"automation":{"tags":[]}});
        let mut old = order.clone();
        old["automation"] = json!({"tags":["x"]});
        old["revision"] = json!(4);
        let mut new = order;
        merge(&mut new, json!({"automation":{"tags":["x"]},"revision":4})).unwrap();
        assert_eq!(new, old);
        let mut fields = json!({"customFields":"broken"});
        let mut old_fields = fields.clone();
        old_fields["customFields"] = json!({});
        old_fields["customFields"]["size"] = json!("L");
        merge(&mut fields, json!({"customFields":{}})).unwrap();
        fields
            .get_mut("customFields")
            .and_then(Value::as_object_mut)
            .unwrap()
            .insert("size".into(), json!("L"));
        assert_eq!(fields, old_fields);
    }
}
