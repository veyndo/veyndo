//! Actor-private workspace preferences; HTTP and MCP share CAS, bounds and the current membership.
use crate::*;
pub(crate) use veyndo_studio::domain::workspace::schema;
use veyndo_studio::store::workspaces;

pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route("/api/studio/work", &[("GET", "read")], get(work))
        .secure_route(
            "/api/studio/workspace",
            &[("GET", "read"), ("PUT", "read")],
            get(read).put(save),
        )
}
async fn identity(a: &App, h: &RequestContext) -> Result<(String, String)> {
    let t = merchant(a, h)?;
    if !h.principal.personal_session {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal merchant session required".into(),
        ));
    }
    auth::mfa::personal(a, h).await?;
    let user = h.principal.user.clone().ok_or(Error(
        StatusCode::FORBIDDEN,
        "A personal merchant session is required".into(),
    ))?;
    if h.principal.app.is_some() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal session required".into(),
        ));
    }
    Ok((t, user))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let (t, user) = identity(a, h).await?;
    if name == "studio.workspace.work.read" {
        return super::work::load(a, h, &t, &user).await;
    }
    match name {
        "studio.workspace.read" => workspaces::read(a, &t, &user).await,
        "studio.workspace.save" => workspaces::save(a, &t, &user, v).await,
        _ => Err(bad("Unknown workspace operation")),
    }
}
async fn read(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "studio.workspace.read", &json!({})).await?,
    ))
}
async fn save(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "studio.workspace.save", &v).await?))
}

async fn work(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "studio.workspace.work.read", &json!({})).await?,
    ))
}
