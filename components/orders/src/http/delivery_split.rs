//! Provider-neutral partial-package allocation; quantities and fiscal order snapshots remain conserved.
use crate::domain::order_fields::order_fields;
use crate::domain::order_patches::{Revised, Workflowed, strip_cart_token};
use crate::domain::order_workflow::{guard, workflow};
use crate::http::transitions::accepted_machine;
use crate::store::machine::machine;
use crate::store::order_mutation;
use crate::types::events::DeliverySplit;
use crate::types::order_document::OrderControl;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::document_slot::field;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, shop_scope::header,
    store::history_context, uid,
};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

/// A shipment split: the delivery to split, the positions that leave it and the order revision shown.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SplitRequest {
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    delivery_index: Option<u64>,
    positions: Value,
}

/// Splits positions of one delivery into a new parcel, conserving quantities.
pub async fn split_delivery(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "orders.write")?;
    permit(&h, "customers.pii")?;
    if veyndo_core::store::environments::parent(&a, &t)
        .await?
        .is_some()
    {
        return Err(bad("Operational shipments cannot be staged"));
    }
    let input: SplitRequest = view(&v);
    let key = header(&h, "idempotency-key")
        .or(input.request_key.as_deref())
        .ok_or(bad("Shipment request key required"))?;
    let fingerprint =
        hash(&json!({"operation":"delivery.split","orderId":id,"body":v}).to_string());
    let (m, mr) = machine(&a.db, &t).await?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, &h, "merchant").await?;
    if let Some(response) = order_mutation::replay(&mut tx, &t, key, &fingerprint).await? {
        return Ok(Json(response));
    }
    let mut o: Value = order_mutation::lock_data(&mut tx, &t, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    let (m, mr) = accepted_machine(&o, m, mr)?;
    let control: OrderControl = view(&o);
    let revision = control.revision.as_u64().unwrap_or(1);
    if !verified_kernel::revision_admissible(revision, input.revision.unwrap_or(0)) {
        return Err(conflict("Order revision changed"));
    }
    let terminal = guard(&o, &m, "delivery", "shipped").is_some();
    let index = input
        .delivery_index
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad("Shipment index required"))?;
    let split_id = uid();
    let (original, parcel) = veyndo_shipping::domain::split::allocate(
        control.deliveries.get(index).unwrap_or(&Value::Null),
        &input.positions,
        terminal,
        &split_id,
    )?;
    let ds = field(&mut o, "deliveries")?
        .as_array_mut()
        .ok_or(bad("Order has no shipments"))?;
    if ds.len() >= 30 {
        return Err(bad("Order shipment limit reached"));
    }
    ds[index] = original;
    ds.push(parcel);
    merge(
        &mut o,
        Revised {
            revision: revision + 1,
        },
    )?;
    order_fields(&mut o);
    let mut event_order = o.clone();
    strip_cart_token(&mut event_order)?;
    let activity = json!({"kind":"delivery","state":"split","sourceIndex":index,"deliveryId":split_id,"positions":input.positions,"requestKey":key,"revision":revision+1});
    let event = DeliverySplit {
        order_id: &id,
        delivery_id: &split_id,
        source_index: index,
        positions: input.positions,
        kind: "delivery",
        state: "split",
        revision: revision + 1,
        request_key: key,
        order: event_order,
    };
    order_mutation::persist(
        &mut tx,
        &t,
        &h,
        &mut o,
        order_mutation::Audit {
            kind: "delivery.split",
            data: activity,
            event_kind: "delivery.split",
            event,
        },
    )
    .await?;
    let mut response = o;
    strip_cart_token(&mut response)?;
    let workflow = workflow(&response, &m, mr, &h);
    merge(&mut response, Workflowed { workflow })?;
    order_mutation::remember(&mut tx, &t, &id, key, &fingerprint, &response).await?;
    tx.commit().await?;
    Ok(Json(response))
}
