//! Order transition transport: revision-checked order, payment and delivery state changes with idempotent replay.
use crate::domain::order_fields::order_fields;
use crate::domain::order_machine::{MachineEdge, OrderMachine};
use crate::domain::order_patches::{Revised, Stated, Workflowed, strip_cart_token};
use crate::domain::order_transitions::{cancellation_blocker, delivery, payment};
use crate::domain::order_workflow::{admitted_edge, guard, workflow};
use crate::store::machine::machine;
use crate::store::order_mutation;
use crate::types::events::StateChanged;
use crate::types::order_document::OrderControl;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::document_slot::field;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, shop_scope::header,
    store::history_context,
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

/// A requested order, payment or delivery transition at the revision it was shown.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct TransitionRequest {
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    tracking_code: Value,
    tracking_url: Value,
    delivery_index: Value,
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
    let input: TransitionRequest = view(&v);
    let request_key = header(&h, "idempotency-key").or(input.request_key.as_deref());
    let fingerprint = hash(&json!({"orderId":id,"body":v}).to_string());
    if let Some(key) = request_key
        && let Some(response) = order_mutation::replay(&mut tx, &t, key, &fingerprint).await?
    {
        return Ok(Json(response));
    }
    let mut o: Value = order_mutation::lock_data(&mut tx, &t, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    // The stored order before this transition, for history and events.
    let from = o.clone();
    let before: OrderControl = view(&from);
    let (machine, machine_revision) = accepted_machine(&o, machine, machine_revision)?;
    let revision = before.revision.as_i64().unwrap_or(1);
    if !verified_kernel::revision_admissible(
        u64::try_from(revision).unwrap_or(0),
        input.revision.unwrap_or(0),
    ) {
        return Err(conflict("Order revision changed"));
    }
    let target = input.state.as_deref().ok_or(bad("State required"))?;
    let kind = input.kind.as_deref().ok_or(bad("Kind required"))?;
    match kind {
        "order" => {
            let current = before.state.as_str().unwrap_or("placed");
            let edge = admitted_edge(&machine, current, target, input.action.as_deref())
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
                merge(field(&mut o, "payment")?, Stated { state: "cancelled" })?;
            }
            merge(&mut o, Stated { state: target })?;
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
    merge(
        &mut o,
        Revised {
            revision: revision + 1,
        },
    )?;
    order_fields(&mut o);
    let event_kind = match kind {
        "order" => "order.state_changed",
        "payment" => "payment.state_changed",
        _ => "delivery.state_changed",
    };
    let mut event_order = o.clone();
    strip_cart_token(&mut event_order)?;
    let from_state = if kind == "order" {
        before.state.clone()
    } else if kind == "payment" {
        before.payment.state.clone()
    } else {
        before
            .delivery(input.delivery_index.as_u64().unwrap_or(0) as usize)
            .state
    };
    order_mutation::persist(&mut tx, &t, &h, &mut o, order_mutation::Audit {
        kind:"transition",
        data:json!({"kind":kind,"state":target,"revision":revision+1,"trackingCode":input.tracking_code,"trackingUrl":input.tracking_url,"deliveryIndex":input.delivery_index,"requestKey":request_key,"lifecycle":if kind=="order" {machine.transitions.iter().find(|e|e.to==target && e.from==before.state).and_then(|e|e.policy.as_ref())} else {None}}),
        event_kind,
        event:StateChanged{order_id:&id,kind,from:from_state.as_str(),state:target,revision:revision+1,request_key,order:event_order},
    }).await?;
    let mut response = o.clone();
    strip_cart_token(&mut response)?;
    let workflow = workflow(&response, &machine, machine_revision, &h);
    merge(&mut response, Workflowed { workflow })?;
    if let Some(key) = request_key {
        order_mutation::remember(&mut tx, &t, &id, key, &fingerprint, &response).await?;
    }
    tx.commit().await?;
    strip_cart_token(&mut o)?;
    Ok(Json(response))
}
