//! Local flow mutations journal the effect in the same transaction; customer authority changes revoke existing sessions.
use crate::domain::flow_text::effective;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Result, State, bad, uid};
use veyndo_shop::store::settings::scoped_locked;

/// One flow step: who it runs as, the language of its note, its idempotency key and its action.
pub struct Step<'a> {
    pub actor: Option<&'a str>,
    pub locale: &'a str,
    pub key: &'a str,
    pub action: &'a str,
}

/// The automation data a flow action changes: the customer's group, the customer or the order, locked for the change.
async fn automation_target(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    customer_id: Option<&str>,
    order: &Value,
    (group, customer): (bool, bool),
) -> Result<Value> {
    Ok(if group {
        let row = sqlx::query_scalar!(
            "SELECT group_name FROM customers WHERE tenant=$1 AND id=$2 FOR UPDATE",
            t,
            customer_id
        )
        .fetch_one(&mut **tx)
        .await?;
        sqlx::query!(
            "INSERT INTO commerce_customer_groups(tenant,id) VALUES($1,$2) ON CONFLICT DO NOTHING",
            t,
            &row
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query_scalar!(
            "SELECT data FROM commerce_customer_groups WHERE tenant=$1 AND id=$2 FOR UPDATE",
            t,
            row
        )
        .fetch_one(&mut **tx)
        .await?
    } else if customer {
        sqlx::query_scalar!(
            "SELECT automation FROM customers WHERE tenant=$1 AND id=$2 FOR UPDATE",
            t,
            customer_id
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(bad("Flow customer unavailable"))?
    } else {
        order.get("automation").cloned().unwrap_or(json!({}))
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
    if let Some(v) = sqlx::query_scalar!(
        "SELECT data FROM order_activity WHERE tenant=$1 AND order_id=$2 AND data->>'flowKey'=$3",
        t,
        id,
        key
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        return Ok(v);
    }
    let mut order: Value = sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE",
        t,
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    let customer_id = view::<OrderRefs>(&order)
        .order_customer
        .customer_id
        .as_str()
        .map(String::from);
    let customer = action.contains("customer");
    let group = action == "action.set.customer.group.custom.field";
    let mut data = automation_target(
        &mut tx,
        t,
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
            sqlx::query!(
                "DELETE FROM order_downloads WHERE tenant=$1 AND order_id=$2",
                t,
                id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query!("INSERT INTO order_downloads(tenant,order_id,asset_id) SELECT $1,$2,a.id FROM product_assets a WHERE a.tenant=$1 AND a.kind='download' AND EXISTS(SELECT 1 FROM jsonb_array_elements($3::jsonb->'cart'->'lineItems') l WHERE l->>'referencedId'=a.product_id) ON CONFLICT DO NOTHING", t, id, &order).execute(&mut *tx).await?;
        }
    } else if action == "action.change.customer.group" || action == "action.change.customer.status"
    {
        let n=sqlx::query!("UPDATE customers SET group_name=COALESCE($1,group_name),active=COALESCE($2,active),revision=revision+1 WHERE tenant=$3 AND id=$4", step.group_id.as_deref(), step.active, t, customer_id.as_deref()).execute(&mut *tx).await?.rows_affected();
        if n != 1 {
            return Err(bad("Flow customer unavailable"));
        }
        sqlx::query!("DELETE FROM customer_sessions WHERE tenant=$1 AND email=(SELECT email FROM customers WHERE tenant=$1 AND id=$2)", t, customer_id.as_deref()).execute(&mut *tx).await?;
        sqlx::query!("UPDATE carts SET token=md5(id||$1),data=jsonb_set(jsonb_set(jsonb_set(jsonb_set(data,'{email}','null'),'{customer_id}','null'),'{group}','\"consumer\"'),'{company}','null'),revision=revision+1 WHERE tenant=$2 AND data->>'customer_id'=$3 AND status='open'", uid(), t, customer_id.as_deref()).execute(&mut *tx).await?;
    } else if action != "note" {
        return Err(bad("Unsupported local action"));
    }
    if group {
        sqlx::query!("UPDATE commerce_customer_groups SET data=$1,revision=revision+1 WHERE tenant=$2 AND id=(SELECT group_name FROM customers WHERE tenant=$2 AND id=$3)", data, t, customer_id.as_deref()).execute(&mut *tx).await?;
    } else if customer
        && action != "action.change.customer.group"
        && action != "action.change.customer.status"
    {
        sqlx::query!(
            "UPDATE customers SET automation=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
            data,
            t,
            customer_id.as_deref()
        )
        .execute(&mut *tx)
        .await?;
    } else if !customer && action != "note" && action != "action.grant.download.access" {
        let revision = view::<OrderRefs>(&order).revision.as_i64().unwrap_or(1);
        merge(
            &mut order,
            json!({ "automation": data, "revision": revision + 1 }),
        )?;
        sqlx::query!(
            "UPDATE orders SET data=$1 WHERE tenant=$2 AND id=$3",
            &order,
            t,
            id
        )
        .execute(&mut *tx)
        .await?;
    }

    let result = json!({"flowKey":key,"action":action,"orderId":id,"text":effective(&step.instruction, locale, &settings.main_locale),"config":config});
    sqlx::query!(
        "INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'flow',$4)",
        t,
        id,
        actor,
        &result
    )
    .execute(&mut *tx)
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
