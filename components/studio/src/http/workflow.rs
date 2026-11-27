//! Revision-bound order workflow configuration used by installed apps and selective sandbox releases.
use crate::store::order_workflow as workflow;
use crate::types::events::WorkflowUpdated;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_platform::store::native_lifecycle::{current_order, validate_order_base};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_marketing::store::configuration_lifecycle::lock as lock_config;
use veyndo_orders::domain::order_machine::{OrderMachine, OrderMachineValidation};
use veyndo_orders::store::machine::machine;
use veyndo_shop::service::read_cache::config;
/// The shop's order workflow with its effective states.
pub async fn get(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "orders.read")?;
    let (m, r) = machine(&a.db, &t).await?;
    let (settings, _) = config(&a, &t).await?;
    let effective = current_order(&a.db, &t).await?;
    Ok(Json(
        json!({"data":m,"revision":r,"mainLocale":settings.main_locale,"effectiveStates":effective.states}),
    ))
}
/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    data: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Saves the workflow at its revision; it never strands an active order state.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SaveInput = view(&v);
    let t = merchant(&h)?;
    permit(&h, "settings.write")?;
    let m: OrderMachine =
        serde_json::from_value(input.data.clone()).map_err(|_| bad("Invalid workflow schema"))?;
    m.validate()?;
    if m.transitions.iter().any(|edge| edge.policy.is_some()) {
        return Err(bad(
            "App transition policies belong to a reviewed app package",
        ));
    }
    let revision = input.revision.ok_or(bad("revision required"))?;
    let mut tx = a.db.begin().await?;
    lock_config(&mut tx, &t).await?;
    workflow::lock(&mut tx, &t).await?;
    if let Some(app) = &m.source_app {
        permit(&h, "apps.manage")?;
        let active = workflow::app_active(&mut tx, &t, app).await?;
        if !active {
            return Err(bad("Workflow source app is not installed"));
        }
    }
    validate_order_base(&mut tx, &t, m.clone()).await?;
    let old = workflow::revision(&mut tx, &t).await?;
    if old.unwrap_or(0) != revision {
        return Err(conflict("Workflow revision changed"));
    }
    // Accepted snapshots retain their original state graph. Legacy orders still need representation.
    // Keep every persisted state represented so changing a definition cannot strand existing orders.
    let states = m.states.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let stranded = workflow::strands_legacy_orders(&mut tx, &t, &states).await?;
    if stranded {
        return Err(conflict("Workflow removes an active order state"));
    }
    workflow::save(&mut tx, &t, &json!(m)).await?;
    let updated = WorkflowUpdated {
        revision: revision + 1,
        actor: header(&h, "x-rac-user"),
        source_app: m.source_app.as_deref(),
    };
    veyndo_core::events::emit(&mut *tx, &t, &updated).await?;
    tx.commit().await?;
    Ok(Json(json!({"saved":true,"revision":revision+1})))
}
