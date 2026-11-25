//! Turns one outbox event into durable flow jobs inside the outbox transaction; a broken flow fails alone, never the whole event.
use crate::domain::flow_projection::{
    job_definition, orderless, projected, public_event, rule_context,
};
use crate::store::{facts, rule_snapshot};
use crate::types::flow::Flow;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{merge, view};
use veyndo_core::{Result, bad, uid};
use veyndo_shop::types::cart::StoredCart;

/// The empty cart an orderless event is evaluated against.
fn event_cart(t: &str) -> Result<StoredCart> {
    Ok(StoredCart {
        id: String::new(),
        tenant: t.into(),
        token: String::new(),
        data: serde_json::from_value(json!({"items":[],"group":"consumer","email":null,"company":null,"session":"app-event","buyer":null,"order":null})).map_err(|_| bad("Invalid event context"))?,
        revision: 0,
        status: "event".into(),
    })
}

/// Records a failed flow job for one flow and event.
async fn failed(
    tx: &mut sqlx::PgConnection,
    t: &str,
    flow: &str,
    event: i64,
    definition: Value,
    error: &str,
) -> Result<()> {
    sqlx::query!("INSERT INTO flow_jobs(id,tenant,flow,event_id,definition,state,error) VALUES($1,$2,$3,$4,$5,'failed',$6) ON CONFLICT(tenant,flow,event_id) DO NOTHING", uid(), t, flow, event, definition, error).execute(&mut *tx).await?;
    Ok(())
}

/// Queues a job for every active flow of the event whose condition matches the frozen rule context.
pub async fn project(
    tx: &mut sqlx::PgConnection,
    t: &str,
    event: i64,
    kind: &str,
    data: &Value,
) -> Result<()> {
    if !projected(kind) {
        return Ok(());
    }
    let data = &public_event(kind, data);
    let rows = sqlx::query!("SELECT id,data FROM commerce_flows WHERE tenant=$1 AND data->>'event'=$2 AND data->>'active'='true' ORDER BY id FOR SHARE", t, kind)
        .fetch_all(&mut *tx)
        .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let subject: EventOrder = view(data);
    let order_id = subject.order_id.as_str().unwrap_or("");
    let order=sqlx::query!("SELECT c.id,c.tenant,c.token,c.data,c.revision,c.status,o.data AS order_data FROM orders o JOIN carts c ON c.id=o.cart_id WHERE o.tenant=$1 AND o.id=$2", t, order_id).fetch_optional(&mut *tx).await?;
    let (cart, q) = if let Some(order) = order {
        (
            super::carts::hydrate(
                order.id,
                order.tenant,
                order.token,
                (order.data, order.revision, order.status),
            )?,
            data.get("order")
                .cloned()
                .unwrap_or_else(|| order.order_data.clone()),
        )
    } else {
        if !orderless(kind) {
            return Ok(());
        }
        (event_cart(t)?, json!({"cart":{}}))
    };
    let mut context = rule_context(kind, data, &q);
    let mut rule_facts = facts::rule_facts(tx, &cart, &context).await?;
    if !order_id.is_empty() {
        facts::order_facts(tx, t, order_id, &q, &mut rule_facts).await?;
    }
    merge(&mut context, json!({ "ruleFacts": rule_facts }))?;
    for row in rows {
        let id: String = row.id.clone();
        let raw: Value = row.data.clone();
        let f: Flow = match serde_json::from_value(raw.clone()) {
            Ok(flow) => flow,
            Err(_) => {
                let definition = json!({"flow":raw,"orderId":order_id});
                failed(tx, t, &id, event, definition, "Invalid stored flow schema").await?;
                continue;
            }
        };
        if !f.active || f.event != kind {
            continue;
        }
        // Reference limits and malformed definitions belong to this flow, not to the whole outbox event.
        let mut frozen_facts = rule_facts.clone();
        let attached = rule_snapshot::attach(tx, t, &raw, &mut frozen_facts).await;
        let mut frozen = context.clone();
        merge(&mut frozen, json!({ "ruleFacts": frozen_facts }))?;
        let matched = match attached {
            Ok(()) => f.condition.checked_matches(&cart, &frozen),
            Err(error) => Err(error),
        };
        match matched {
            Err(error) => {
                let definition = json!({"flow":f,"orderId":order_id});
                failed(tx, t, &id, event, definition, &error.1).await?;
            }
            Ok(true) => {
                let definition = job_definition(
                    &json!(f),
                    kind,
                    data,
                    order_id,
                    &q,
                    &json!(cart.data),
                    &frozen,
                );
                sqlx::query!("INSERT INTO flow_jobs(id,tenant,flow,event_id,definition) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,flow,event_id) DO NOTHING", uid(), t, &id, event, definition).execute(&mut *tx).await?;
            }
            Ok(false) => {}
        }
    }
    Ok(())
}

/// The order an outbox event is about.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EventOrder {
    order_id: Value,
}
