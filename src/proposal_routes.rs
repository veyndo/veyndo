//! HTTP proposal creation, approval and task listing.
use crate::{App, Json, Path, RequestContext, Result, State, Value, agent, apply, bad, merchant};

pub(crate) use veyndo_studio::http::proposals::{agent_plan, tasks};
pub(crate) async fn agent_apply(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    agent::assistant_access(&h)?;
    let t = merchant(&a, &h)?;
    if v["approve"] != true {
        return Err(bad("Explicit approve=true required"));
    }
    Ok(Json(apply(&a, &t, &id, &h).await?))
}
