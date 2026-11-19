//! Provider-neutral package admission freezes allocation before a paid carrier effect; approved apps reconcile only their own intent.
use super::*;
use veyndo_shipping::domain::booking;
pub(crate) async fn booking_intent(
    a: &App,
    h: &RequestContext,
    m: &apps::Manifest,
    operation: &str,
    v: &Value,
) -> Result<Value> {
    let t = merchant(a, h)?;
    for right in ["orders.write", "documents.create", "customers.pii"] {
        auth::permit(h, right)?;
    }
    if h.principal.app.as_deref() != Some(m.id.as_str()) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Carrier intent requires the approved app callback identity".into(),
        ));
    }
    if staging::parent(a, &t).await?.is_some() {
        return Err(bad("Operational carrier intents cannot be staged"));
    }
    let id = v["id"].as_str().ok_or(bad("Order required"))?;
    let index = v["deliveryIndex"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad("Shipment index required"))?;
    let key = v["requestKey"]
        .as_str()
        .filter(|s| (8..=128).contains(&s.len()))
        .ok_or(bad("Carrier request key required"))?;
    let intent =
        json!({"sourceApp":m.id,"requestKey":key,"allocationDigest":v["allocationDigest"]});
    let (machine, _) = commerce::machine(&a.db, &t).await?;
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, h, "app").await?;
    apps::lock_package_write(&mut tx, &t, m).await?;
    let mut order: Value =
        sqlx::query_scalar("SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(&t)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    if v["allocationDigest"] != allocation_digest(&order, index) {
        return Err(conflict(
            "Package allocation changed; no carrier booking admitted",
        ));
    }
    let closed = operation == "delivery_reserve"
        && commerce::guard(&order, &machine, "delivery", "shipped").is_some();
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
    commerce::order_mutation::persist(&mut tx,&t,h,&mut order,commerce::order_mutation::Audit{
        kind:event,data:json!({"kind":"delivery","state":operation,"deliveryIndex":index,"sourceApp":m.id}),
        event_kind:event,event:json!({"orderId":id,"deliveryIndex":index,"sourceApp":m.id}),
    }).await?;
    tx.commit().await?;
    Ok(
        json!({"reserved":operation=="delivery_reserve","released":operation=="delivery_release","reused":false}),
    )
}
