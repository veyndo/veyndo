//! Revisioned hosted frontend addresses point to an existing tenant-owned Experience; origin selection remains operator-only.
use crate::ports::HostedInstall;
use crate::store::frontend_bindings as stored;
use crate::store::locks;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::{lenient, view};
use veyndo_core::host_names::{links, valid_origin};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, validate_tenant,
};
use veyndo_identity::permit;
use veyndo_marketing::store::configuration_lifecycle::lock;

/// A hosted frontend binding: its address, channel, hosted app and experience address.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BindingRequest {
    #[serde(deserialize_with = "lenient")]
    alias: Option<String>,
    #[serde(deserialize_with = "lenient")]
    channel: Option<String>,
    #[serde(deserialize_with = "lenient")]
    app_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    experience_alias: Option<String>,
}
/// The JSON fields `bind` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BindInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Points a revisioned frontend address at a channel and an Experience of this shop; the operator chooses the origin, a managed app is installed in the same transaction.
pub async fn bind(
    install: &dyn HostedInstall,
    a: &State,
    h: &RequestContext,
    v: &Value,
) -> Result<Value> {
    let args: BindInput = view(v);
    permit(h, "settings.write")?;
    let t = tenant(h)?;
    let input: BindingRequest = view(v);
    let alias = input
        .alias
        .as_deref()
        .ok_or(bad("Frontend address required"))?;
    validate_tenant(alias)?;
    if [
        "app",
        "www",
        "api",
        "admin",
        "mail",
        "platform",
        "experience",
    ]
    .contains(&alias)
    {
        return Err(bad("Reserved address"));
    }
    let channel = input
        .channel
        .as_deref()
        .filter(|s| identifier(s))
        .ok_or(bad("Channel required"))?;
    let requested_app = input.app_id.as_deref();
    if v.get("appId").is_some() && requested_app != Some("storyfront") {
        return Err(bad("Unknown hosted app"));
    }
    let mut app_id = requested_app.map(str::to_owned);
    let experience = input.experience_alias.as_deref().unwrap_or(alias);
    validate_tenant(experience)?;
    let origin = std::env::var("HOSTED_FRONTEND_ORIGIN").map_err(|_| {
        Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Hosted frontend service is not configured".into(),
        )
    })?;
    if !valid_origin(&origin) {
        return Err(bad("Operator frontend origin is invalid"));
    }
    // Address allocation is a global native registry operation: it must see
    // collisions in other shops and install the managed app in this transaction.
    // Every subsequent read/write retains the already admitted owning tenant.
    let mut tx = a.db.begin_privileged().await?;
    lock(&mut tx, &t).await?;
    locks::advisory(&mut tx, alias, 43).await?;
    let occupied = stored::names_other_shop(&mut tx, alias, &t).await?;
    if occupied {
        return Err(conflict("Address belongs to another shop"));
    }
    let available = stored::channel_exists(&mut tx, &t, channel).await?;
    if !available {
        return Err(bad("Channel required in this shop"));
    }
    // Additional domains can only select an Experience already mounted by this same tenant.
    if experience != alias {
        let own = stored::experience_app(&mut tx, &t, experience).await?;
        let Some(own) = own else {
            return Err(bad("Experience is not connected to this shop"));
        };
        if app_id.is_none() {
            app_id = own;
        }
    }
    let row = stored::lock(&mut tx, alias).await?;
    if let Some(row) = &row {
        if row.tenant != t {
            return Err(conflict("Address belongs to another shop"));
        }
        if app_id.is_none() {
            app_id = row.app_id.clone();
        }
        let unchanged =
            row.channel == channel && row.experience_alias == experience && row.app_id == app_id;
        if !unchanged && args.revision != Some(row.revision) {
            return Err(conflict("Frontend revision changed"));
        }
        if unchanged {
            if let Some(id) = &app_id {
                install.ensure(&mut tx, &t, id).await?;
            }
            tx.commit().await?;
            return Ok(
                json!({"alias":alias,"channel":channel,"revision":row.revision,"urls":links(alias)}),
            );
        }
    } else if args.revision.is_some_and(|r| r != 0) {
        return Err(conflict("Frontend revision changed"));
    }
    if let Some(id) = &app_id {
        install.ensure(&mut tx, &t, id).await?;
    }
    let revision = stored::save(
        &mut tx,
        stored::NewBinding {
            alias,
            tenant: &t,
            channel,
            origin: &origin,
            experience,
            app_id: app_id.as_deref(),
        },
    )
    .await?;
    veyndo_core::store::outbox::publish_on(&mut *tx, &t, "frontend.configured", &json!({"alias":alias,"channel":channel,"revision":revision,"actor":header(h,"x-rac-user")})).await?;
    tx.commit().await?;
    Ok(json!({"alias":alias,"channel":channel,"revision":revision,"urls":links(alias)}))
}

/// The JSON fields `remove` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RemoveInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Disconnects a frontend address at its current revision.
pub async fn remove(a: &State, h: &RequestContext, alias: &str, v: &Value) -> Result<Value> {
    let input: RemoveInput = view(v);
    permit(h, "settings.write")?;
    let t = tenant(h)?;
    let revision = input.revision.ok_or(bad("Frontend revision required"))?;
    let mut tx = a.db.begin().await?;
    lock(&mut tx, &t).await?;
    let n = stored::remove(&mut tx, &t, alias, revision).await?;
    if n == 0 {
        return Err(conflict("Frontend absent or revision changed"));
    }
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        &t,
        "frontend.disconnected",
        &json!({"alias":alias,"actor":header(h,"x-rac-user")}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"disconnected":true,"alias":alias}))
}
