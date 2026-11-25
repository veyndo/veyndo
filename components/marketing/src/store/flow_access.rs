//! Every queued flow step rehydrates current membership; stored definitions never preserve revoked privileges. A stage's live shop and the shop's activity are resolved by core.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::store::{environments, shop_activity};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
pub async fn headers(a: &State, t: &str, actor: Option<&str>) -> Result<RequestContext> {
    headers_on(&mut *a.db.begin().await?, t, actor).await
}
/// App admission already holds configuration locks; reuse that connection for current grants.
pub async fn headers_on(
    conn: &mut PgConnection,
    t: &str,
    actor: Option<&str>,
) -> Result<RequestContext> {
    let mut h = RequestContext::new();
    h.insert(
        "x-tenant",
        t.parse().map_err(|_| bad("Invalid flow tenant"))?,
    );
    h.insert("x-rac-tenant", t.parse().unwrap());
    let actor = actor.ok_or(bad("Flow actor required"))?;
    h.insert(
        "x-rac-user",
        actor.parse().map_err(|_| bad("Invalid flow actor"))?,
    );
    if actor == "bootstrap" {
        h.insert("x-rac-role", "owner".parse().unwrap());
    } else {
        let live = environments::live_parent(&mut *conn, t).await?;
        let r=sqlx::query!(r#"SELECT role AS "role!",permissions AS "permissions!" FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active"#, live.as_deref().unwrap_or(t), actor).fetch_optional(conn).await?.ok_or(Error(StatusCode::FORBIDDEN,"Flow owner lost access".into()))?;
        h.insert(
            "x-rac-role",
            r.role
                .clone()
                .parse()
                .map_err(|_| bad("Invalid flow role"))?,
        );
        let permissions: Value = r.permissions.clone();
        if !permissions.is_null() {
            h.insert(
                "x-rac-permissions",
                permissions
                    .to_string()
                    .parse()
                    .map_err(|_| bad("Invalid flow permissions"))?,
            );
        }
    }
    veyndo_identity::permit(&h, "automation.manage")?;
    Ok(h)
}

/// A scheduled proposal uses current Flow rights, never a synthetic personal session.
pub async fn proposal_headers(
    a: &State,
    t: &str,
    actor: Option<&str>,
    job: &str,
) -> Result<RequestContext> {
    let mut h = headers(a, t, actor).await?;
    h.insert(
        "x-rac-flow-job",
        job.parse().map_err(|_| bad("Invalid Flow job"))?,
    );
    veyndo_identity::assistant_access(&h)?;
    veyndo_identity::permit(&h, "catalog.write")?;
    veyndo_identity::permit(&h, "knowledge.read")?;
    let row = shop_activity::activity(a, t).await?;
    if row.agents_stopped
        || row.status.clone() != "active"
        || row
            .preview_owner
            .clone()
            .is_some_and(|owner| Some(owner.as_str()) != actor || row.valid != Some(true))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Flow agent or workspace is unavailable".into(),
        ));
    }
    Ok(h)
}
