//! Exactly-once local observation projection; associations retain order/event evidence and simulation labels.
use crate::knowledge::{sync_observation, sync_product};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_catalog::store::products;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, State, bad, hash};

/// The JSON fields `project` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProjectInput {
    #[serde(deserialize_with = "lenient")]
    order_id: Option<String>,
}

/// The order fields observation reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ObservedOrder {
    #[serde(deserialize_with = "lenient")]
    payment: ObservedPayment,
    #[serde(deserialize_with = "lenient")]
    cart: ObservedCart,
}

/// Whether the order charged real money.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ObservedPayment {
    real_money_charged: Value,
}

/// The order's line items.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ObservedCart {
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
}

/// The product a line item refers to.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ObservedLine {
    #[serde(deserialize_with = "lenient")]
    referenced_id: Option<String>,
}

/// Projects one order event once; `external` tells whether an order waits for a confirmed capture.
pub async fn project(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    event: i64,
    kind: &str,
    data: &Value,
    external: fn(&Value) -> bool,
) -> Result<()> {
    let input: ProjectInput = view(data);
    if kind != "order.placed" && kind != "payment.captured" {
        return Ok(());
    }
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,12))", t)
        .execute(&mut **tx)
        .await?;
    let fresh = sqlx::query!(
        "INSERT INTO knowledge_receipts(tenant,event_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
        t,
        event
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if fresh == 0 {
        return Ok(());
    }
    let id = input
        .order_id
        .as_deref()
        .ok_or(bad("Order event lacks ID"))?;
    let order = sqlx::query_scalar!("SELECT data FROM orders WHERE tenant=$1 AND id=$2", t, id)
        .fetch_one(&mut **tx)
        .await?;
    // External pending orders are observed only after confirmed capture. Legacy demo orders stay labelled.
    if external(&order) && kind != "payment.captured" {
        return Ok(());
    }
    let observed: ObservedOrder = view(&order);
    let simulated = observed.payment.real_money_charged != true;
    let mut ids = observed
        .cart
        .line_items
        .ok_or(bad("Invalid order positions"))?
        .iter()
        .filter_map(|v| view::<ObservedLine>(v).referenced_id)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    for id in &ids {
        // runtime SQL: the row goes to veyndo_catalog::store::products::product, which maps a PgRow.
        if let Some(p) = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
        {
            sync_product(&mut *tx, t, &json!(products::product(&p))).await?;
        }
    }
    for (i, left) in ids.iter().enumerate() {
        for right in ids.iter().skip(i + 1) {
            let fresh=sqlx::query!("INSERT INTO pair_evidence(tenant,left_id,right_id,order_id,event_id,simulated) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
                t, left, right, id, event, simulated).execute(&mut **tx).await?.rows_affected();
            if fresh == 0 {
                continue;
            }
            let n=sqlx::query_scalar!("INSERT INTO observed_pairs(tenant,left_id,right_id,orders,last_event) VALUES($1,$2,$3,1,$4) ON CONFLICT(tenant,left_id,right_id) DO UPDATE SET orders=observed_pairs.orders+1,last_event=EXCLUDED.last_event,updated_at=now() RETURNING orders",
                t, left, right, event).fetch_one(&mut **tx).await?;
            sync_observation(&mut *tx, t, left, right, n, event).await?;
            let hypothesis = hash(&format!("bundle:{left}:{right}"));
            sqlx::query!("INSERT INTO knowledge_hypotheses(tenant,id,kind,evidence) VALUES($1,$2,'bundle-experiment',$3) ON CONFLICT(tenant,id) DO UPDATE SET evidence=EXCLUDED.evidence,updated_at=now(),revision=knowledge_hypotheses.revision+1",
                t, hypothesis, json!({"left":left,"right":right,"observedOrders":n,"lastEvent":event,"causalUpliftProven":false,"suggestion":"Test a bundle against a randomized comparison; review stock and margin first"})).execute(&mut **tx).await?;
        }
    }
    Ok(())
}
/// What order observation has recorded: pairs, hypotheses and the number of processed events.
pub async fn observations(a: &State, t: &str) -> Result<Value> {
    let rows=sqlx::query!(r#"SELECT p.left_id,p.right_id,p.orders,p.last_event,p.updated_at::text AS "updated_at!",(SELECT count(*) FROM pair_evidence e WHERE e.tenant=p.tenant AND e.left_id=p.left_id AND e.right_id=p.right_id AND e.simulated) AS "simulated!" FROM observed_pairs p WHERE tenant=$1 ORDER BY orders DESC,left_id,right_id LIMIT 24"#,
        t).fetch_all(&a.db).await?;
    let hypotheses=sqlx::query!("SELECT id,kind,state,evidence,revision FROM knowledge_hypotheses WHERE tenant=$1 ORDER BY updated_at DESC LIMIT 24",
        t).fetch_all(&a.db).await?;
    let processed = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM knowledge_receipts WHERE tenant=$1"#,
        t
    )
    .fetch_one(&a.db)
    .await?;
    Ok(
        json!({"processedEvents":processed,"modelWeightsUpdated":false,"causalUpliftProven":false,"limit":24,"pairs":rows.iter().map(|r|json!({"left":r.left_id,"right":r.right_id,"orders":r.orders,"simulatedOrders":r.simulated,"source":"order-observation","lastEvent":r.last_event,"updatedAt":r.updated_at})).collect::<Vec<_>>(),"hypotheses":hypotheses.iter().map(|r|json!({"id":r.id,"kind":r.kind,"state":r.state,"revision":r.revision,"evidence":r.evidence})).collect::<Vec<_>>()}),
    )
}
/// The associations a merchant published; no order counts or identities.
pub async fn public_pairs(a: &State, t: &str) -> Result<Value> {
    let rows=sqlx::query!(r#"SELECT evidence->>'left' AS "left_id!",evidence->>'right' AS "right_id!" FROM knowledge_hypotheses WHERE tenant=$1 AND state='published' ORDER BY updated_at DESC LIMIT 24"#,
        t).fetch_all(&a.db).await?;
    Ok(json!(rows.iter().map(|r|json!({"left":r.left_id,"right":r.right_id,"source":"merchant-approved-order-association","causalUpliftProven":false})).collect::<Vec<_>>()))
}
