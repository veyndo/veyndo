//! One permission-aware action gateway serves HTTP, UI and MCP; service egress is operator configured.
use super::*;
pub(crate) fn validate_input(schema: &Value, v: &Value) -> Result<()> {
    input_schema::validate(schema, v)
}
pub(crate) fn validate_native_input(schema: &Value, v: &Value) -> Result<()> {
    input_schema::validate_native(schema, v)
}
pub(crate) async fn invoke_app(
    a: &App,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
) -> Result<Value> {
    invoke(a, h, id, name, v, None).await
}
pub(super) async fn invoke(
    a: &App,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
    mcp: Option<mcp_views::Mode<'_>>,
) -> Result<Value> {
    let start = std::time::Instant::now();
    let result = execute(a, h, id, name, v, mcp).await;
    let status = result
        .as_ref()
        .map(|_| StatusCode::OK)
        .unwrap_or_else(|e| e.0);
    observability::record(a, h, id, name, v, status, start.elapsed().as_millis()).await;
    result
}
// A job callback holds its admitted manifest/action through enqueue. Re-reading
// the package must never turn a formerly declared job into a service invocation.
pub(super) async fn invoke_job(
    a: &App,
    h: &RequestContext,
    m: &Manifest,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let action = m
        .actions
        .iter()
        .find(|action| action.name == name && action.handler == "job")
        .ok_or(bad("Callback requires a declared job action"))?;
    action_admission::check(a, h, action, v)?;
    tool_contract::checked_output(action, jobs::enqueue(a, h, m, action, v).await?)
}
async fn execute(
    a: &App,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
    mcp: Option<mcp_views::Mode<'_>>,
) -> Result<Value> {
    let t = tenant(h)?;
    let m = package(a, &t, id, true).await?;
    let action = m
        .actions
        .iter()
        .find(|act| act.name == name)
        .ok_or(bad("Unknown app action"))?;
    if let Some(mode) = mcp {
        mcp_views::admit_tool(a, h, &m, action, &t, mode)?;
    }
    action_admission::check(a, h, action, v)?;
    if let Some(result) = super::hosted::action(a, &t, id, name).await? {
        return tool_contract::checked_output(action, result);
    }
    let e = action
        .entity
        .as_ref()
        .and_then(|n| m.entities.iter().find(|e| e.name == *n));
    if let Some(e) = e {
        core_extensions::access(e, h, action.handler == "save")?;
    }
    let result = match action.handler.as_str() {
        "assets" | "asset_preview" => {
            auth::permit(h, "catalog.read")?;
            assets::app_file_callback(a, h, &action.handler, v).await
        }
        "asset_upload" => Err(bad(
            "Asset uploads require the bounded multipart surface endpoint",
        )),
        "list" => {
            data::list_scoped(
                a,
                &t,
                &m,
                e.ok_or(bad("Entity required"))?,
                v,
                action.public.then(|| marketing::channel_id(h)),
            )
            .await
        }
        "submit" => submissions::submit(a, h, &m, e.ok_or(bad("Entity required"))?, v).await,
        "save" => data::save(a, &t, &m, e.ok_or(bad("Entity required"))?, v, h).await,
        "configure" => configure(a, h, id, v).await,
        "configurations" => {
            merchant(a, h)?;
            configurations(a, &t, id, v).await
        }
        "payment_command" => crate::payments::app_command(a, h, id, v).await,
        "payment_onboarding" => crate::payments::onboarding(a, h, id, v).await,
        "service" => {
            if !m.permissions.contains(&"service.call".into()) || m.runtime != "service" {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Service permission required".into(),
                ));
            }
            let result = service_call(a, &t, id, &format!("actions/{name}"), v).await?;
            if result["purgeSources"] == true && m.permissions.contains(&"knowledge.write".into()) {
                purge_sources(a, &t, id, result["exportCursor"].as_i64().unwrap_or(0)).await?;
            }
            Ok(result)
        }
        "job" => jobs::enqueue(a, h, &m, action, v).await,
        "emit" => {
            if v.to_string().len() > 16384 {
                return Err(bad("App event exceeds limit"));
            }
            let event: i64 = sqlx::query_scalar(
                "INSERT INTO outbox(tenant,kind,data) VALUES($1,$2,$3) RETURNING id",
            )
            .bind(&t)
            .bind(format!("app.{id}.{name}"))
            .bind(v)
            .fetch_one(&a.db)
            .await?;
            Ok(json!({"eventId":event,"accepted":true}))
        }
        _ => Err(bad("Unsupported app action")),
    }?;
    tool_contract::checked_output(action, result)
}
pub(crate) async fn service_call(
    a: &App,
    t: &str,
    id: &str,
    path: &str,
    v: &Value,
) -> Result<Value> {
    veyndo_app_platform::service::service_call::service_call(
        &service_limits::platform(a),
        t,
        id,
        path,
        v,
    )
    .await
}

pub(crate) fn ui_url(id: &str) -> Option<String> {
    veyndo_app_platform::domain::service_ui::ui_url(id, &crate::runtime_config::get().services)
}

/// The registry uses the same live permission check as direct actions.
pub(super) fn action_authorized(_a: &App, h: &RequestContext, action: &Action) -> bool {
    veyndo_app_platform::domain::action_admission::authorized(h, action)
}
pub(crate) use veyndo_app_platform::domain::action_admission::additional as additional_permissions;
