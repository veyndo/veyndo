//! Search all installed app capabilities with current rights; data reads reuse their exact admitted native action.
use super::*;
pub(crate) use veyndo_app_platform::domain::discovery::{CAPABILITIES, relevant, schema};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/agent/apps/search",
            &[("POST", "apps.read")],
            post(search),
        )
        .secure_route("/api/agent/apps/read", &[("POST", "apps.read")], post(read))
}
async fn search(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "merchant.apps.search", &v).await?))
}
async fn read(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "merchant.apps.read", &v).await?))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "apps.read")?;
    validate_native_input(
        &schema(name).ok_or(bad("Unknown app discovery operation"))?,
        v,
    )?;
    if name == "merchant.apps.read" {
        let m = package(a, &t, v["app"].as_str().unwrap(), true).await?;
        let action = m
            .actions
            .iter()
            .find(|act| {
                act.name == v["action"]
                    && admitted(a, h, &m, act)
                    && ["list", "configurations"].contains(&act.handler.as_str())
            })
            .ok_or(bad("App action is not an admitted managed read"))?;
        return Box::pin(invoke_app(a, h, &m.id, &action.name, &v["arguments"])).await;
    }
    let rows =
        sqlx::query("SELECT manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id")
            .bind(&t)
            .fetch_all(&a.db)
            .await?;
    let query = v["query"].as_str().unwrap_or("");
    let mut apps = vec![];
    for row in rows {
        let m: Manifest = serde_json::from_value(row.get("manifest"))
            .map_err(|_| bad("Invalid installed app"))?;
        let actions=m.actions.iter().filter(|act|admitted(a,h,&m,act) && ["list","configurations"].contains(&act.handler.as_str()))
            .map(|act|json!({"action":act.name,"description":act.description,"inputSchema":act.input_schema})).take(12).collect::<Vec<_>>();
        let proposals=m.actions.iter().filter(|act|admitted(a,h,&m,act) && act.handler=="save").map(|act|json!({"name":format!("app.{}.{}",m.id,act.name),"description":act.description,"inputSchema":act.input_schema,"reviewRequired":true})).take(12).collect::<Vec<_>>();
        if actions.is_empty() && proposals.is_empty() {
            continue;
        }
        let score = relevant(&m, query);
        if !query.trim().is_empty() && score == 0 {
            continue;
        }
        apps.push((
            score,
            m.id.clone(),
            json!({"app":m.id,"version":m.version,"name":m.name,"tools":actions,"proposals":proposals}),
        ));
    }
    apps.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
    let offset = match v["after"].as_str() {
        Some(id) => apps
            .iter()
            .position(|x| x.1 == id)
            .map(|n| n + 1)
            .ok_or(conflict("App discovery changed; search again"))?,
        None => 0,
    };
    let limit = v["limit"].as_u64().unwrap_or(8) as usize;
    let has_more = offset + limit < apps.len();
    let page = apps
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|x| x.2)
        .collect::<Vec<_>>();
    Ok(
        json!({"nextAfter":if has_more {page.last().map(|p|p["app"].clone())}else{None},"hasMore":has_more,"apps":page}),
    )
}
fn admitted(_a: &App, h: &RequestContext, m: &Manifest, action: &Action) -> bool {
    veyndo_app_platform::domain::discovery::admitted(h, m, action)
}
