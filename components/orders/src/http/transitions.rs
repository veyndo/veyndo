//! Order transition transport: revision-checked order, payment and delivery state changes with idempotent replay.
use crate::domain::order_fields::order_fields;
use crate::domain::order_machine::{MachineEdge, OrderMachine};
use crate::domain::order_transitions::{cancellation_blocker, delivery, payment};
use crate::domain::order_workflow::{guard, workflow};
use crate::store::machine::machine;
use crate::store::order_mutation;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, history_context,
    shop_scope::header,
};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

use veyndo_core::model_contract::error as contract;
/// The machine the order was accepted under, or the shop's current one.
pub(crate) fn accepted_machine(
    o: &Value,
    fallback: OrderMachine,
    revision: i64,
) -> Result<(OrderMachine, i64)> {
    veyndo_app_model::native_lifecycle_snapshot::order_machine(o, fallback, revision)
        .map_err(contract)
}
/// Why the current actor and facts refuse this edge, if they do.
fn reason(h: &RequestContext, edge: &MachineEdge, o: &Value) -> Option<&'static str> {
    veyndo_app_model::native_lifecycle::reason(edge, o, |p| permit(h, p).is_ok())
}

/// Moves an order, its payment or one delivery to a new state under the accepted lifecycle.
pub async fn transition_order(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "orders.write")?;
    permit(&h, "customers.pii")?;
    let (machine, machine_revision) = machine(&a.db, &t).await?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, &h, "merchant").await?;
    let request_key = header(&h, "idempotency-key").or_else(|| v["requestKey"].as_str());
    let fingerprint = hash(&json!({"orderId":id,"body":v}).to_string());
    if let Some(key) = request_key
        && let Some(response) = order_mutation::replay(&mut tx, &t, key, &fingerprint).await?
    {
        return Ok(Json(response));
    }
    let r = sqlx::query("SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&t)
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    let mut o: Value = r.get("data");
    let (machine, machine_revision) = accepted_machine(&o, machine, machine_revision)?;
    let revision = o["revision"].as_i64().unwrap_or(1);
    if !verified_kernel::revision_admissible(
        u64::try_from(revision).unwrap_or(0),
        v["revision"].as_u64().unwrap_or(0),
    ) {
        return Err(conflict("Order revision changed"));
    }
    let target = v["state"].as_str().ok_or(bad("State required"))?;
    let kind = v["kind"].as_str().ok_or(bad("Kind required"))?;
    match kind {
        "order" => {
            let current = o["state"].as_str().unwrap_or("placed");
            let edge = machine
                .transitions
                .iter()
                .find(|e| {
                    e.from == current
                        && e.to == target
                        && v["action"].as_str().is_none_or(|id| id == e.id)
                })
                .ok_or(conflict("Invalid order transition"))?;
            let native = guard(&o, &machine, kind, target);
            let policy = reason(&h, edge, &o);
            if !verified_kernel::native_lifecycle_admissible(
                native.is_none(),
                policy != Some("lifecyclePermission"),
                policy != Some("lifecycleGuard"),
            ) {
                return Err(Error(
                    if policy == Some("lifecyclePermission") {
                        StatusCode::FORBIDDEN
                    } else {
                        StatusCode::CONFLICT
                    },
                    if policy == Some("lifecyclePermission") {
                        "lifecyclePermission"
                    } else {
                        native.or(policy).unwrap_or("Invalid order transition")
                    }
                    .into(),
                ));
            }
            if target == "cancelled" {
                if let Some(reason) = cancellation_blocker(&o) {
                    return Err(conflict(reason));
                }
                veyndo_b2b::store::purchase::release_budget(&mut tx, &t, &id).await?;
                veyndo_inventory::store::reservations::release(&mut tx, &t, &id).await?;
                o["payment"]["state"] = json!("cancelled");
            }
            o["state"] = json!(target);
        }
        "payment" => {
            if let Some(reason) = guard(&o, &machine, kind, target) {
                return Err(conflict(reason));
            }
            payment(&mut o, &machine, target)?;
        }
        "delivery" => {
            if let Some(reason) = guard(&o, &machine, kind, target) {
                return Err(conflict(reason));
            }
            delivery(&mut o, &v, target)?;
        }
        _ => return Err(bad("Kind must be payment or delivery")),
    }
    o["revision"] = json!(revision + 1);
    order_fields(&mut o);
    let event_kind = match kind {
        "order" => "order.state_changed",
        "payment" => "payment.state_changed",
        _ => "delivery.state_changed",
    };
    let from = r.get::<Value, _>("data");
    let mut event_order = o.clone();
    if let Some(cart) = event_order["cart"].as_object_mut() {
        cart.remove("token");
    }
    order_mutation::persist(&mut tx, &t, &h, &mut o, order_mutation::Audit {
        kind:"transition",
        data:json!({"kind":kind,"state":target,"revision":revision+1,"trackingCode":v["trackingCode"],"trackingUrl":v["trackingUrl"],"deliveryIndex":v["deliveryIndex"],"requestKey":request_key,"lifecycle":if kind=="order" {machine.transitions.iter().find(|e|e.to==target && e.from==from["state"]).and_then(|e|e.policy.as_ref())} else {None}}),
        event_kind,
        event:json!({"orderId":id,"kind":kind,"from":if kind=="order" {from["state"].clone()} else if kind=="payment" {from["payment"]["state"].clone()} else {from["deliveries"][v["deliveryIndex"].as_u64().unwrap_or(0) as usize]["state"].clone()},"state":target,"revision":revision+1,"requestKey":request_key,"order":event_order}),
    }).await?;
    let mut response = o.clone();
    if let Some(cart) = response["cart"].as_object_mut() {
        cart.remove("token");
    }
    response["workflow"] = workflow(&response, &machine, machine_revision, &h);
    if let Some(key) = request_key {
        order_mutation::remember(&mut tx, &t, &id, key, &fingerprint, &response).await?;
    }
    tx.commit().await?;
    if let Some(cart) = o["cart"].as_object_mut() {
        cart.remove("token");
    }
    Ok(Json(response))
}
