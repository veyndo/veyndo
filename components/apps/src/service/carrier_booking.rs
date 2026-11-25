//! Provider-neutral package admission freezes allocation before a paid carrier effect; approved apps reconcile only their own intent.
use crate::store::package_fence::lock_package_write;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::{lenient, view};
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, store::history_context,
};
use veyndo_identity::{merchant, permit};
use veyndo_orders::domain::order_workflow::guard;
use veyndo_orders::store::carrier_documents::allocation_digest;
use veyndo_orders::store::machine::machine;
use veyndo_orders::store::order_mutation;
use veyndo_shipping::domain::booking;

/// The package a carrier app books: the order, the shipment, the request key and the allocation it saw.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CarrierIntent {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    delivery_index: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
    allocation_digest: Value,
}
/// The JSON fields `booking_intent` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BookingIntentInput {
    allocation_digest: Value,
}

/// Reserves or releases a carrier booking for the approved app that owns the intent.
pub async fn booking_intent(
    a: &State,
    h: &RequestContext,
    m: &Manifest,
    operation: &str,
    v: &Value,
) -> Result<Value> {
    let args: BookingIntentInput = view(v);
    let t = merchant(h)?;
    for right in ["orders.write", "documents.create", "customers.pii"] {
        permit(h, right)?;
    }
    if h.principal.app.as_deref() != Some(m.id.as_str()) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Carrier intent requires the approved app callback identity".into(),
        ));
    }
    if veyndo_core::store::environments::parent(a, &t)
        .await?
        .is_some()
    {
        return Err(bad("Operational carrier intents cannot be staged"));
    }
    let input: CarrierIntent = view(v);
    let id = input.id.as_deref().ok_or(bad("Order required"))?;
    let index = input
        .delivery_index
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad("Shipment index required"))?;
    let key = input
        .request_key
        .as_deref()
        .filter(|s| (8..=128).contains(&s.len()))
        .ok_or(bad("Carrier request key required"))?;
    let intent =
        json!({"sourceApp":m.id,"requestKey":key,"allocationDigest":input.allocation_digest});
    let (machine, _) = machine(&a.db, &t).await?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "app").await?;
    lock_package_write(&mut tx, &t, m).await?;
    let mut order: Value =
        sqlx::query_scalar("SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(&t)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    if args.allocation_digest != allocation_digest(&order, index) {
        return Err(conflict(
            "Package allocation changed; no carrier booking admitted",
        ));
    }
    let closed =
        operation == "delivery_reserve" && guard(&order, &machine, "delivery", "shipped").is_some();
    let parcel = order["deliveries"]
        .as_array_mut()
        .and_then(|ds| ds.get_mut(index))
        .ok_or(bad("Shipment unavailable"))?;
    let event = match booking::transition(parcel, operation == "delivery_reserve", closed, &intent)?
    {
        booking::Step::Settled(answer) => return Ok(answer),
        booking::Step::Changed(event) => event,
    };
    order["revision"] = json!(order["revision"].as_u64().unwrap_or(1) + 1);
    order_mutation::persist(&mut tx,&t,h,&mut order,order_mutation::Audit{
        kind:event,data:json!({"kind":"delivery","state":operation,"deliveryIndex":index,"sourceApp":m.id}),
        event_kind:event,event:json!({"orderId":id,"deliveryIndex":index,"sourceApp":m.id}),
    }).await?;
    tx.commit().await?;
    Ok(
        json!({"reserved":operation=="delivery_reserve","released":operation=="delivery_release","reused":false}),
    )
}
