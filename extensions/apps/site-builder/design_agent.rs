//! Scoped visual proposals reuse central inference and native revision-bound tasks; models cannot publish or execute.
use crate::{SiteHost, SiteState};
use axum::Json;
use axum::Router;
use axum::extract::FromRef;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_app_platform::store::managed_data::list_page;
use veyndo_app_platform::store::packages::package;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::admission::reserve_ai_attempt;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::database::lease::Lease;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::parent;
use veyndo_identity::route_policy::SecureRoutes;
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::language::language_context;
mod agent_handoff;
mod proposals;
const DESIGN_PROMPT: &str = include_str!("prompts/site-design.v1.txt");
pub(crate) fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    SiteState: FromRef<S>,
{
    Router::new()
        .secure_route(
            "/api/merchant/site/task",
            &[("POST", "channels.manage")],
            post(task_route),
        )
        .secure_route(
            "/api/merchant/site/propose",
            &[("POST", "channels.manage")],
            post(propose_route),
        )
        .secure_route(
            "/api/merchant/site/plan",
            &[("POST", "channels.manage")],
            post(plan_route),
        )
        .secure_route(
            "/api/merchant/site/apply",
            &[("POST", "channels.manage")],
            post(apply_route),
        )
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024))
}
/// The input schema of a design operation.
pub fn schema(name: &str) -> Option<Value> {
    let p = match name {
        "merchant.site.plan" => {
            json!({"id":{"type":"string"},"revision":{"type":"integer"},"selected":{"type":"string"},"instruction":{"type":"string","minLength":1,"maxLength":2000},"urls":{"type":"array","maxItems":3,"items":{"type":"string"}},"images":{"type":"array","maxItems":2,"items":{"type":"string"}},"style":{"type":"boolean"},"structure":{"type":"boolean"},"content":{"type":"boolean"}})
        }
        "merchant.site.task" => {
            json!({"id":{"type":"string"},"revision":{"type":"integer"},"instruction":{"type":"string","maxLength":2000},"selected":{"type":"string"},"agent":{"type":"string","enum":["codex","claude_code"]}})
        }
        "merchant.site.propose" => {
            json!({"id":{"type":"string"},"revision":{"type":"integer"},"instruction":{"type":"string","maxLength":2000},"selected":{"type":"string"},"summary":{"type":"string","maxLength":500},"fields":{"type":"object"}})
        }
        "merchant.site.apply" => json!({"taskId":{"type":"string"},"approve":{"type":"boolean"}}),
        _ => return None,
    };
    Some(
        json!({"type":"object","properties":p,"additionalProperties":false,"required":if name.ends_with("apply"){vec!["taskId","approve"]}else if name.ends_with("propose"){vec!["id","revision","fields","instruction"]}else{vec!["id","revision","instruction"]}}),
    )
}
/// The input of a design operation: the saved page and revision, the instruction, the selected block,
/// what the change may touch, and for agent results the returned fields and summary.
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct DesignRequest {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    task_id: Option<String>,
    id: Value,
    revision: Value,
    instruction: Value,
    selected: Value,
    agent: Value,
    #[serde(deserialize_with = "lenient")]
    structure: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    content: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    style: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    fields: Option<serde_json::Map<String, Value>>,
    #[serde(deserialize_with = "lenient")]
    summary: Option<String>,
}

impl DesignRequest {
    fn structure(&self) -> bool {
        self.structure == Some(true)
    }
    fn content(&self) -> bool {
        self.content == Some(true)
    }
    fn style(&self) -> bool {
        self.style == Some(true)
    }
}

/// One design operation on the saved page of a design sandbox: plan alternatives, export a coding-agent task, propose agent results or apply an approved proposal.
pub async fn invoke(
    site: &dyn SiteHost,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let a = site.core();
    let t = merchant(h)?;
    permit(h, "channels.manage")?;
    permit(h, "channels.read")?;
    permit(h, "assistant.use")?;
    if parent(a, &t).await?.is_none() {
        return Err(bad(
            "Create or select a design sandbox before preparing changes",
        ));
    }
    let input: DesignRequest = view(v);
    if name == "merchant.site.apply" {
        if input.approve != Some(true) {
            return Err(bad("Explicit approval required"));
        }
        let id = input.task_id.as_deref().ok_or(bad("Proposal required"))?;
        let proposal =
            sqlx::query_scalar::<_, Value>("SELECT proposal FROM tasks WHERE tenant=$1 AND id=$2")
                .bind(&t)
                .bind(id)
                .fetch_optional(&a.db)
                .await?
                .ok_or(Error(StatusCode::NOT_FOUND, "Proposal unavailable".into()))?;
        if proposal["source"] != "site.designer"
            || proposal["proposal"]["app_action"]["app"] != "site_builder"
        {
            return Err(bad("Not a shop design proposal"));
        }
        return site.apply(&t, id, h).await;
    }
    if ![
        "merchant.site.plan",
        "merchant.site.task",
        "merchant.site.propose",
    ]
    .contains(&name)
    {
        return Err(bad("Unknown design operation"));
    }
    if input
        .id
        .as_str()
        .is_none_or(|s| s.is_empty() || s.len() > 100)
        || input.revision.as_i64().is_none_or(|n| n < 1)
    {
        return Err(bad("Saved page ID and positive revision required"));
    }
    let instruction = input
        .instruction
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 2000)
        .ok_or(bad("Describe a design change in 1..2000 bytes"))?;
    let m = package(a, &t, "site_builder", true).await?;
    let entity = m
        .entities
        .iter()
        .find(|e| e.name == "pages")
        .ok_or(bad("Pages unavailable"))?;
    let current = list_page(a, &t, &m, entity, &json!({"id":input.id,"limit":1})).await?;
    let row = current["elements"]
        .as_array()
        .and_then(|r| r.first())
        .ok_or(bad("Save the page before asking the agent"))?;
    if row["revision"] != input.revision {
        return Err(conflict("Page changed; reload before preparing a design"));
    }
    let selected = input.selected.as_str().filter(|s| !s.is_empty());
    let mut fields = row.clone();
    fields.as_object_mut().unwrap().remove("id");
    fields.as_object_mut().unwrap().remove("revision");
    if name == "merchant.site.task" {
        return agent_handoff::export(site, h, &fields, &input).await;
    }
    if name == "merchant.site.propose" {
        return agent_handoff::propose(site, h, (&m, entity), &fields, &input, instruction).await;
    }
    if selected.is_some() && !input.structure() && !input.content() {
        return Err(bad(
            "Choose the whole page for theme changes, or enable content or structure for the selected section",
        ));
    }
    let (sources, images) = site.references(v).await?;
    let mut schema = json!({"type":"object","additionalProperties":false,"required":["interpretation","variants"],"properties":{"interpretation":{"type":"string","maxLength":2000},"variants":{"type":"array","minItems":2,"maxItems":3,"items":{"type":"object","additionalProperties":false,"required":["summary","content_json","theme_json"],"properties":{"summary":{"type":"string","maxLength":500},"content_json":{"type":"string","maxLength":8192},"theme_json":{"type":"string","maxLength":1024}}}}}});
    if !input.structure() && !input.content() {
        schema["properties"]["variants"]["items"]["properties"]["content_json"] =
            json!({"type":"string","enum":["{}"]});
    }
    let registry = site.surfaces(h, true).await?;
    let elements = registry["surfaces"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["surface"]["location"] == "storefront.block")
        .take(64)
        .map(|s| json!({"app":s["app"],"surface":s["surface"]["id"],"label":s["surface"]["label"]}))
        .collect::<Vec<_>>();
    let prompt = format!(
        "Instruction: {instruction}\nSelected block: {selected:?}. Current fields: {fields}. Registered app elements: {elements:?}. App blocks use kind=app, extension={{app,surface}} from this registry only. Reference observations: {}. Apply reference style={}, structure={}, content={}. Preserve product IDs and facts. Return the same content/theme schema, no extra keys. If a block is selected, content_json must have blocks with only that one existing ID. text uses localized maps, rich uses localized safe rich-description blocks. Theme supports accent/background/foreground hex colors, font=sans/serif/mono, radius 0..32, spacing 12..96, columns 1..4. Missing references are unavailable, not evidence. Explain analysis and differences in {}.",
        sources,
        input.style(),
        input.structure(),
        input.content(),
        language_context(a, h).await?.0
    );
    reserve_ai_attempt(a, &t).await?;
    let models = site.models();
    let _slot = models.slots.clone().try_acquire_owned().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Design model busy; retry shortly".into(),
        )
    })?;
    let _lease = Lease::acquire(a, &t, "model", 2).await?;
    let output = models
        .inference
        .structured_with_images(None, DESIGN_PROMPT, &prompt, &schema, &images)
        .await
        .map_err(|_| {
            Error(
                StatusCode::BAD_GATEWAY,
                "Design provider unavailable; your page is unchanged".into(),
            )
        })?;
    if output.value["variants"]
        .as_array()
        .is_none_or(|rows| !(2..=3).contains(&rows.len()))
    {
        return Err(bad("Design provider must return two or three alternatives"));
    }
    proposals::prepare(
        a,
        h,
        &m,
        entity,
        &fields,
        &input,
        proposals::ResultContext {
            instruction,
            output: &output.value,
            sources: &sources,
            model: &output.model,
            usage: &output.usage,
        },
    )
    .await
}

async fn plan_route(
    State(SiteState(site)): State<SiteState>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&*site, &h, "merchant.site.plan", &v).await?))
}
async fn apply_route(
    State(SiteState(site)): State<SiteState>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&*site, &h, "merchant.site.apply", &v).await?))
}

async fn task_route(
    State(SiteState(site)): State<SiteState>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&*site, &h, "merchant.site.task", &v).await?))
}
async fn propose_route(
    State(SiteState(site)): State<SiteState>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&*site, &h, "merchant.site.propose", &v).await?))
}
