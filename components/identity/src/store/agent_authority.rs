//! Saved agent runs retain an upper authority bound; every continuation rechecks credential, membership and private stage admission.
use crate::{allowed, assistant_access};
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_kernel::verified_kernel;

/// The saved request of an agent run, as the rebuilt context reads it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RunRequest {
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    channel: Option<String>,
}
/// Rebuilds the request context of a saved run in its own privileged transaction.
pub async fn headers(a: &State, row: &sqlx::postgres::PgRow) -> Result<RequestContext> {
    let mut tx = a.db.begin_privileged().await?;
    let result = headers_on(&mut tx, row).await?;
    tx.commit().await?;
    Ok(result)
}
/// Recheck current identity in a caller-owned native authority connection.
pub async fn headers_on(
    connection: &mut sqlx::PgConnection,
    row: &sqlx::postgres::PgRow,
) -> Result<RequestContext> {
    let t: String = row.get("tenant");
    let actor: String = row.get("run_actor");
    let mut h = RequestContext::new();
    h.insert(
        "x-tenant",
        t.parse().map_err(|_| bad("Invalid saved tenant"))?,
    );
    h.principal.tenant = Some(t.clone());
    h.principal.user = Some(actor.clone());
    h.principal.personal_session = row.get("run_personal");
    {
        if actor == "bootstrap" {
            if !veyndo_core::process_config::get().allow_bootstrap {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Bootstrap agent authority disabled".into(),
                ));
            }
            h.principal.role = Some("owner".into());
        } else {
            let digest: Option<String> = row.get("run_credential");
            let valid:bool=sqlx::query_scalar!(r#"SELECT CASE WHEN $3 THEN EXISTS(SELECT 1 FROM user_sessions WHERE digest=$1 AND user_id=$2 AND expires_at>statement_timestamp()) ELSE EXISTS(SELECT 1 FROM integration_keys WHERE digest=$1 AND user_id=$2 AND tenant=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$4),$4) AND app_id IS NULL AND expires_at>statement_timestamp()) END AS "valid!""#, digest, &actor, h.principal.personal_session, &t).fetch_one(&mut *connection).await?;
            if !valid {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Agent credential expired or revoked".into(),
                ));
            }
            let member=sqlx::query!(r#"SELECT role AS "role!",permissions AS "permissions!" FROM membership_grants WHERE tenant=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) AND user_id=$2 AND active"#, &t, &actor).fetch_optional(&mut *connection).await?.ok_or(Error(StatusCode::FORBIDDEN,"Agent owner lost membership".into()))?;
            h.principal.role = Some(member.role);
            let permissions: Value = member.permissions;
            h.principal.permissions = (!permissions.is_null()).then(|| permissions.to_string());
            let scopes: Value = row.get("run_scopes");
            if scopes
                .as_array()
                .is_none_or(|s| s.iter().any(|s| s.as_str().is_none_or(|s| !allowed(&h, s))))
            {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Agent owner permissions changed; start a new reviewed run".into(),
                ));
            }
            if !h.principal.personal_session {
                h.principal.agent_key=sqlx::query_scalar!("SELECT id FROM integration_keys WHERE digest=$1 AND user_id=$2 AND agent_budget IS NOT NULL AND expires_at>statement_timestamp()", row.get::<Option<String>,_>("run_credential"), &actor).fetch_optional(&mut *connection).await?;
                if h.principal.agent_key.is_none() {
                    return Err(Error(
                        StatusCode::FORBIDDEN,
                        "Dedicated agent budget required".into(),
                    ));
                }
                let current:Value=sqlx::query_scalar!("SELECT permissions FROM integration_keys WHERE digest=$1 AND user_id=$2 AND expires_at>statement_timestamp()", row.get::<Option<String>,_>("run_credential"), &actor).fetch_one(&mut *connection).await?;
                let mut key = h.clone();
                key.principal.role = Some("admin".into());
                key.principal.permissions = Some(current.to_string());
                if scopes
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s.as_str().is_none_or(|scope| !allowed(&key, scope)))
                {
                    return Err(Error(
                        StatusCode::FORBIDDEN,
                        "Integration permissions narrowed".into(),
                    ));
                }
            }
        }
        let stopped: bool =
            veyndo_core::store::tenants::agents_stopped(&mut *connection, &t).await?;
        if stopped {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Shop agents are stopped by an administrator".into(),
            ));
        }
        h.principal.role = Some("admin".into());
        h.principal.permissions = Some(row.get::<Value, _>("run_scopes").to_string());
        let request: RunRequest = view(&row.get::<Value, _>("run_request"));
        h.insert(
            "x-commerce-locale",
            request
                .locale
                .as_deref()
                .unwrap_or("en-GB")
                .parse()
                .map_err(|_| bad("Invalid saved locale"))?,
        );
        if let Some(channel) = request.channel.as_deref() {
            h.insert(
                "sw-sales-channel-id",
                channel.parse().map_err(|_| bad("Invalid saved channel"))?,
            );
        }
        let admitted=sqlx::query!("SELECT t.status,e.preview_owner,e.preview_until>statement_timestamp() AS valid FROM tenants t LEFT JOIN shop_environments e ON e.tenant=$1 WHERE t.id=coalesce(e.live_tenant,$1)", &t).fetch_optional(&mut *connection).await?.ok_or(bad("Shop no longer exists"))?;
        let owner: Option<String> = admitted.preview_owner;
        if !verified_kernel::agent_preview_admissible(
            admitted.status == "active",
            owner.is_some(),
            h.principal.personal_session,
            owner.as_deref() == Some(actor.as_str()),
            admitted.valid == Some(true),
        ) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Agent shop or personal preview is unavailable".into(),
            ));
        }
        assistant_access(&h)?;
        Ok(h)
    }
}
