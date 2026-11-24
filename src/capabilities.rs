//! Shared HTTP/MCP capability dispatch and tool authorization.
use crate::agent::retrieve;
use crate::cart_mutation::set_items;
use crate::cart_price::cart_json;
use crate::cart_storage::load_cart;
use crate::catalog_routes::catalog_page;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::localization::language_context;
use crate::marketing;
use crate::order_checkout::checkout;
use crate::order_routes::orders;
use crate::proposal_apply::apply;
use crate::{agent_runs, apps, auth, cognition, commerce};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tracing::Instrument;
use veyndo_checkout::store::cart_create::new_cart_context;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
use veyndo_intelligence::knowledge;

mod admission;
mod catalog;
pub(crate) use veyndo_core::tool_contract::ToolContract;
mod dispatch;
mod registry;
mod schema;
pub(crate) use registry::{OPERATIONS, tool_schema};
pub(crate) use registry::{agent_reads, export};
pub(crate) async fn capabilities() -> Json<Value> {
    Json(
        json!({"capabilities":OPERATIONS.iter().map(|operation|operation.description()).collect::<Vec<_>>()}),
    )
}

/// Select a boxed domain future before polling it, keeping inactive domains off the worker stack.
pub(crate) fn invoke<'a>(
    a: &'a App,
    h: &'a RequestContext,
    name: &'a str,
    v: &'a Value,
) -> CapabilityFuture<'a> {
    if name.starts_with("app.") {
        if h.principal.agent_key.is_some() {
            return Box::pin(async {
                Err(Error(
                    StatusCode::FORBIDDEN,
                    "Budgeted planning agents use native read tools only".into(),
                ))
            });
        }
        return Box::pin(async move {
            let (id, action) = name
                .strip_prefix("app.")
                .unwrap()
                .split_once('.')
                .ok_or(bad("Invalid app capability name"))?;
            apps::invoke_app(a, h, id, action, v).await
        });
    }
    let Some(operation) = registry::find(name) else {
        return Box::pin(async { Err(bad("Unknown capability")) });
    };
    if !operation.admitted(a, h) {
        return Box::pin(async {
            Err(Error(
                StatusCode::FORBIDDEN,
                "Operation is not currently authorized".into(),
            ))
        });
    }
    if let Err(error) = apps::validate_native_input(&operation.input_schema(), v) {
        return Box::pin(async move { Err(error) });
    }
    Box::pin(
        async move {
            let value = operation.owner.invoke(a, h, name, v).await?;
            operation.check_output(value)
        }
        .instrument(
            tracing::info_span!("operation",operation=name,trace_id=%veyndo::trace::current()),
        ),
    )
}

/// The arguments a shopper or merchant capability takes from its call.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CapabilityArgs {
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    query: Option<String>,
    #[serde(deserialize_with = "lenient")]
    session: Option<String>,
    items: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient", rename = "idempotency_key")]
    idempotency_key: Option<String>,
    #[serde(deserialize_with = "lenient")]
    instruction: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient", rename = "task_id")]
    task_id: Option<String>,
}

pub(super) type CapabilityFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>>;

pub(super) async fn invoke_native(
    a: &App,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    if name.starts_with("knowledge.") {
        merchant(a, h)?;
        auth::permit(h, "knowledge.read")?;
    }
    if name == "merchant.apply" {
        auth::permit(h, "catalog")?;
    }
    let args: CapabilityArgs = view(v);
    match name {
        "catalog.search" => {
            let criteria =
                serde_json::from_value(v.clone()).map_err(|_| bad("Invalid catalog criteria"))?;
            let Json(result) = catalog_page(State(a.clone()), h.clone(), criteria).await?;
            Ok(result)
        }
        "catalog.detail" => {
            let criteria =
                serde_json::from_value(v.clone()).map_err(|_| bad("Invalid variant criteria"))?;
            let Json(v) = commerce::product_detail(
                State(axum::extract::FromRef::from_ref(a)),
                h.clone(),
                Path(args.product_id.clone().ok_or(bad("Product ID required"))?),
                axum::extract::Query(criteria),
            )
            .await?;
            Ok(v)
        }
        "checkout.options" => {
            let Json(v) = commerce::options(State(a.clone()), h.clone()).await?;
            Ok(v)
        }
        "checkout.select" => {
            let Json(v) =
                commerce::select_checkout(State(a.clone()), h.clone(), Json(v.clone())).await?;
            Ok(v)
        }
        "knowledge.external" => Ok(
            json!({"elements":apps::private_evidence(a,&tenant(h)?,args.query.as_deref().unwrap_or("")).await?}),
        ),
        "knowledge.graph" => Ok(knowledge::graph(&a.db, &tenant(h)?).await?),
        "knowledge.search" => retrieve(a, &tenant(h)?, args.query.as_deref().unwrap_or("")).await,
        "cart.create" => {
            let c = new_cart_context(a, h, args.session.as_deref().unwrap_or(""), "mcp").await?;
            cart_json(a, &c).await
        }
        "cart.quote" => cart_json(a, &load_cart(a, h).await?).await,
        "cart.offer" => cognition::signed_offer::issue_offer(a, h).await,
        "offer.verify" => cognition::signed_offer::verify_offer(a, h, v).await,
        "cart.replace" => {
            let i = serde_json::from_value(args.items.clone()).map_err(|e| bad(e.to_string()))?;
            let c = set_items(
                a,
                h,
                i,
                Some(args.revision.ok_or(bad("revision required"))?),
            )
            .await?;
            cart_json(a, &c).await
        }
        "checkout.complete" => {
            checkout(
                a,
                h,
                args.idempotency_key
                    .as_deref()
                    .ok_or(bad("idempotency_key required"))?,
            )
            .await
        }
        "merchant.plan" => {
            auth::permit(h, "knowledge.read")?;
            merchant(a, h)?;
            let (locale, _) = language_context(a, h).await?;
            let instruction = args.instruction.as_deref().unwrap_or("");
            if instruction.is_empty() || instruction.len() > 4000 {
                return Err(bad("Instruction must contain 1..4000 characters"));
            }
            let queued=agent_runs::enqueue(a,h,json!({"instruction":instruction,"locale":locale,"channel":marketing::channel_id(h)}),None).await?;
            let run = agent_runs::wait(a, h, queued["taskId"].as_str().unwrap()).await?;
            agent_runs::plan_result(run)
        }
        "merchant.apply" => {
            let t = merchant(a, h)?;
            if args.approve != Some(true) {
                return Err(bad("approve=true required"));
            }
            apply(
                a,
                &t,
                args.task_id.as_deref().ok_or(bad("task_id required"))?,
                h,
            )
            .await
        }
        "merchant.orders" => {
            let Json(v) = orders(State(a.clone()), h.clone()).await?;
            Ok(v)
        }
        _ => Err(bad("Unknown capability")),
    }
}
