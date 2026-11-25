//! Reviewed work binds bounded native configuration commands and managed app saves before any worker effect; approval re-binds and queues it.
use super::work_execution::validate_translation;
use crate::domain::proposal::{NATIVE, Step};
use crate::domain::views::{Evidence, ProposalParts};
use crate::ports::WorkHost;
use crate::store::{products, tasks};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_app_platform::domain::planning::AppChange;
use veyndo_catalog::domain::product_admission::{admit_product_write, product_write_guidance};
use veyndo_catalog::service::categories::admit as admit_category;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::model_contract::validate_native;
use veyndo_core::shop_scope::header;
use veyndo_core::store::environments::parent;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, hash};
use veyndo_identity::allowed;
use veyndo_intelligence::store::agent_runs::claim_conversation;

/// A native operation the planner may propose, with the product write contract for product operations.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationEntry {
    name: &'static str,
    input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    product_contract: Option<Value>,
}

/// The native configuration operations the actor may propose, with their schemas and the product write contract.
pub fn catalogue<H: WorkHost>(host: &H, h: &RequestContext) -> Vec<Value> {
    host.work_operations(h)
        .into_iter()
        .filter(|(name, _)| NATIVE.contains(name))
        .map(|(name, schema)| {
            json!(OperationEntry {
                name,
                input_schema: schema,
                product_contract: name
                    .starts_with("merchant.product.")
                    .then(product_write_guidance),
            })
        })
        .collect()
}
/// The native targets and revision of a reviewed work step's arguments.
#[derive(Deserialize, Default)]
#[serde(default)]
struct WorkArguments {
    product: Value,
    category: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}
/// A product or category target's identity and revisions.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Target {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    content_revision: Option<i64>,
}
/// Binds reviewed work to its current targets: app saves to their revision, native steps to their admitted schema and stable targets.
pub async fn bind<H: WorkHost>(
    host: &H,
    t: &str,
    h: &RequestContext,
    steps: &mut [Step],
) -> Result<()> {
    let a = host.models().core;
    if steps.is_empty() {
        return Ok(());
    }
    if steps.len() > 12 || json!(steps).to_string().len() > 16384 {
        return Err(bad("Work plan exceeds 12 steps or 16 KiB"));
    }
    if parent(a, t).await?.is_none() {
        return Err(bad("Agent configuration work requires a private workspace"));
    }
    let mut created_products: Vec<String> = Vec::new();
    for step in steps.iter_mut() {
        step.binding = None;
        if step.label.is_empty() || step.label.len() > 200 || step.arguments_json.len() > 8192 {
            return Err(bad("Invalid work step"));
        }
        let args: Value = serde_json::from_str(&step.arguments_json)
            .map_err(|_| bad("Work arguments must be JSON"))?;
        if let Some(target) = step.name.strip_prefix("app.") {
            let (app, action) = target
                .split_once('.')
                .ok_or(bad("Invalid work app action"))?;
            let mut change = AppChange {
                app: app.into(),
                action: action.into(),
                arguments_json: step.arguments_json.clone(),
                expected_app_revision: 0,
            };
            host.bind_change(t, &mut change, h).await?;
            step.arguments_json = change.arguments_json.clone();
            step.binding = Some(change);
        } else {
            let schema = host
                .work_operations(h)
                .into_iter()
                .find(|(name, _)| *name == step.name && NATIVE.contains(name))
                .map(|(_, schema)| schema)
                .ok_or(bad(
                    "Operation is not admitted for private configuration work",
                ))?;
            validate_native(&schema, &args)?;
            let work: WorkArguments = view(&args);
            if step.name.starts_with("merchant.product.") {
                admit_product_write(&work.product, step.name.ends_with(".create"))?;
            }
            if step.name == "merchant.translations.create" {
                validate_translation(host, t, &args, &created_products).await?;
                continue;
            }
            if step.name.starts_with("merchant.category.") {
                admit_category(
                    &host.catalog(),
                    t,
                    &work.category,
                    step.name.ends_with(".create"),
                )
                .await?;
            }
            if step.name.ends_with(".create") {
                let key = if step.name == "merchant.product.create" {
                    "product"
                } else {
                    "category"
                };
                let target: Target = view(if key == "product" {
                    &work.product
                } else {
                    &work.category
                });
                let id = target
                    .id
                    .as_deref()
                    .filter(|id| identifier(id) || uuid::Uuid::parse_str(id).is_ok())
                    .ok_or(bad("Work creation needs an explicit stable identifier"))?;
                let exists = if key == "product" {
                    products::exists(a, t, id).await?
                } else {
                    products::category_exists(a, t, id).await?
                };
                if key == "product" {
                    if created_products.iter().any(|existing| existing == id) {
                        return Err(bad("Duplicate product create in reviewed work"));
                    }
                    created_products.push(id.to_owned());
                }
                if exists {
                    return Err(conflict("Planned create target already exists"));
                }
            } else if step.name == "merchant.product.save" {
                let product: Target = view(&work.product);
                if product.revision.is_none() && product.content_revision.is_none() {
                    return Err(bad("Planned product edit needs its original revision"));
                }
            } else if step.name == "merchant.category.save" {
                if view::<Target>(&work.category).revision.is_none() {
                    return Err(bad("Planned category edit needs its original revision"));
                }
            } else if work.revision.is_none() {
                return Err(bad("Planned edit needs its original revision"));
            }
        }
    }
    if json!(steps).to_string().len() > 24576 {
        return Err(bad("Bound work exceeds 24 KiB"));
    }
    Ok(())
}
/// Personal approval re-binds the exact reviewed work and queues its execution under the live-shop stop lock.
pub async fn approve<H: WorkHost>(
    host: &H,
    t: &str,
    id: &str,
    h: &RequestContext,
) -> Result<Value> {
    let a = host.models().core;
    if !h.principal.personal_session && header(h, "x-rac-user") != Some("bootstrap") {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal approval required for work execution".into(),
        ));
    }
    // Rebinding uses native owners which acquire their own connection. Do not hold
    // a transaction while calling them: transaction poolers may have one backend.
    let row = tasks::get(a, t, id).await?;
    if row.applied {
        return Ok(json!({"taskId":id,"applied":true,"replayed":true}));
    }
    if row.run_actor != header(h, "x-rac-user").unwrap_or("") {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Only the original actor may approve work".into(),
        ));
    }
    let ceiling = row.run_scopes;
    if ceiling.as_array().is_none_or(|scopes| {
        scopes
            .iter()
            .any(|scope| scope.as_str().is_none_or(|scope| !allowed(h, scope)))
    }) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Current permissions no longer cover the reviewed work".into(),
        ));
    }
    let mut request = row.run_request;
    if request.get("execution").is_some() {
        let state = row.run_state.unwrap_or_default();
        if !["queued", "running"].contains(&state.as_str()) {
            return Err(conflict(
                "Work is stopped; use its explicit continuation or cancellation controls",
            ));
        }
        return Ok(json!({"taskId":id,"applied":false,"queued":true,"replayed":true}));
    }
    if row.run_state.as_deref() != Some("ready") {
        return Err(conflict("Work proposal is not ready"));
    }
    let evidence = row.proposal;
    let mut steps: Vec<Step> = serde_json::from_value(
        view::<ProposalParts>(&view::<Evidence>(&evidence).proposal).operations,
    )
    .map_err(|_| bad("Invalid saved work plan"))?;
    if steps.is_empty() {
        return Err(bad("Work plan has no steps"));
    }
    // Rebinding may only confirm the exact original review, never refresh stale revisions silently.
    let reviewed = json!(steps);
    bind(host, t, h, &mut steps).await?;
    if reviewed != json!(steps) {
        return Err(conflict("Work targets changed after review"));
    }
    // The native coordinator reads the live-shop stop flag outside the private
    // tenant's SQL visibility, as enqueue/resume do. Actor-scoped reads and exact
    // rebinding above authorize this one task. Lock live control before its task,
    // matching the global-stop owner and never widening the worker's data scope.
    let mut tx = a.db.begin_privileged().await?;
    let stopped: bool = veyndo_core::store::tenants::agents_stopped_locked(&mut *tx, t).await?;
    if stopped {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Shop agents are stopped by an administrator".into(),
        ));
    }
    let revision = tasks::lock_revision(&mut tx, t, id).await?;
    if revision != row.run_revision {
        return Err(conflict(
            "Work changed while checking approval; reload its current state",
        ));
    }
    if let Some(conversation) = row.conversation_id {
        claim_conversation(&mut tx, t, &conversation, id).await?;
    }
    merge(&mut request, json!({ "execution": steps }))?;
    merge(&mut request, json!({ "history": Value::Null }))?;
    let credential = header(h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(hash);
    tasks::queue_execution(
        &mut tx,
        t,
        id,
        &request,
        credential,
        h.principal.personal_session,
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"taskId":id,"applied":false,"queued":true}))
}
