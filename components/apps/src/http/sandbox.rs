//! Sandbox transparency, explicit version rollback and typed port calls of one installed app; declarations come from the stored package, activity from the existing metadata-only call and record history.
use crate::domain::action_policy::readonly;
use crate::domain::app_facts::Described;
use crate::domain::approval;
use crate::domain::consent::actor_permissions;
use crate::domain::package_contract::{Contract, facts};
use crate::domain::port_wiring::{Installed, resolve};
use crate::ports::Platform;
use crate::service::app_ports;
use crate::store::{activity, package_contracts, package_install, packages::package};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad, process_config};
use veyndo_identity::{merchant, permit};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Publisher<'a> {
    publisher: &'a str,
    key_id: &'a str,
    channel: &'a str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Egress {
    service: bool,
    approved: bool,
    origins: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Model<'a> {
    entity: &'a str,
    fields: usize,
    public: bool,
    /// The core object this model extends, such as an order.
    #[serde(skip_serializing_if = "Option::is_none")]
    extends: Option<veyndo_app_model::Object>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tool<'a> {
    name: &'a str,
    #[serde(flatten)]
    described: Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionRow<'a> {
    version: &'a str,
    digest: &'a str,
    installed_at: &'a str,
    current: bool,
    rollback: bool,
}

/// The origin an operator configured for an app service, without credentials or paths.
fn origins(service: &Value) -> Vec<String> {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Service {
        #[serde(deserialize_with = "lenient")]
        url: Option<String>,
    }
    view::<Service>(service)
        .url
        .and_then(|u| reqwest::Url::parse(&u).ok())
        .map(|u| u.origin().ascii_serialization())
        .into_iter()
        .collect()
}
/// What the package declares it may do; pure over the stored package and the active contracts.
fn declared(m: &Manifest, contract: Option<&Contract>, active: &[(String, Contract)]) -> Value {
    let services = &process_config::get().services;
    let service = m.runtime == "service" && m.permissions.iter().any(|p| p == "service.call");
    let installed: Vec<Installed<'_>> = active
        .iter()
        .map(|(app, contract)| Installed { app, contract })
        .collect();
    let consumes: Vec<Value> = contract
        .map(|c| c.consumes.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|p| {
            let provider = resolve(&m.id, p, &installed)
                .map(|(app, port)| json!({"app":app,"version":port.version}));
            json!({"name":p.name,"version":p.version,"provider":provider})
        })
        .collect();
    let provides: Vec<Value> = contract
        .map(|c| c.provides.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|p| json!({"name":p.name,"version":p.version,"action":p.action}))
        .collect();
    json!({
        "permissions": m.permissions,
        "egress": Egress {
            service,
            approved: service && approval::approved(m, &services[&m.id]),
            origins: if service { origins(&services[&m.id]) } else { vec![] },
        },
        "data": m.entities.iter().map(|e| Model { entity: &e.name, fields: e.fields.len(), public: e.public_read, extends: e.extends.as_ref().map(|x| x.object) }).collect::<Vec<_>>(),
        "ports": {"provides": provides, "consumes": consumes},
        "tools": m.actions.iter().map(|a| {
            let (f, declared) = facts(m, a, contract);
            Tool { name: &a.name, described: json!(Described::of(&f, declared || readonly(a))) }
        }).collect::<Vec<_>>(),
    })
}

/// What an installed app may do and what it recently did; metadata only, never payloads or secrets.
pub async fn sandbox(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &id, false).await?;
    let mut tx = a.db.begin().await?;
    crate::store::locks::tenant_scope(&mut tx, &t).await?;
    let contract = package_contracts::of_app(&mut *tx, &t, &id).await?;
    let active = package_contracts::active(&mut *tx, &t).await?;
    let versions = package_contracts::versions(&mut tx, &t, &id).await?;
    tx.rollback().await?;
    let services: Vec<String> = m
        .actions
        .iter()
        .filter(|a| a.handler == "service")
        .map(|a| a.name.clone())
        .collect();
    let calls = activity::calls(&a, &t, &id).await?;
    let egress = activity::calls_of(&a, &t, &id, &services).await?;
    let writes = activity::writes(&a, &t, &id).await?;
    let rollbacks = contract
        .as_ref()
        .map(|c| c.rollbacks.as_slice())
        .unwrap_or_default();
    let versions: Vec<VersionRow<'_>> = versions
        .iter()
        .map(|v| VersionRow {
            version: &v.version,
            digest: &v.digest,
            installed_at: &v.installed_at,
            current: v.version == m.version,
            rollback: rollbacks.iter().any(|r| r.to_version == v.version),
        })
        .collect();
    Ok(Json(json!({
        "app": m.id,
        "version": m.version,
        "publisher": m.distribution.as_ref().map(|d| Publisher { publisher: &d.publisher, key_id: &d.key_id, channel: &d.channel }),
        "declared": declared(&m, contract.as_ref(), &active),
        "observed": {
            "calls": calls,
            "egress": egress.iter().map(|c| json!({"id":c.id,"action":c.action,"status":c.status,"at":c.at})).collect::<Vec<_>>(),
            "writes": writes.iter().map(|w| json!({"entity":w.entity,"record":w.record,"revision":w.revision,"source":w.source,"at":w.at})).collect::<Vec<_>>(),
        },
        "versions": versions,
    })))
}

/// The JSON fields `rollback` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RollbackInput {
    #[serde(deserialize_with = "lenient")]
    version: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
}
/// Explicitly reinstalls an earlier published version of this shop with the declared data steps.
pub async fn rollback(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: RollbackInput = view(&v);
    permit(&h, "users")?;
    let t = merchant(&h)?;
    if input.approve != Some(true) {
        return Err(bad(
            "Approve the rollback; app data is converted to the earlier version",
        ));
    }
    let version = input.version.ok_or(bad("version required"))?;
    let mut conn = a.db.begin().await?;
    crate::store::locks::tenant_scope(&mut conn, &t).await?;
    let (target, _) = package_contracts::version(&mut conn, &t, &id, &version).await?;
    conn.rollback().await?;
    // The earlier version was consented to once; the current actor must still hold its rights.
    actor_permissions(&h, &target)?;
    Ok(Json(
        package_install::rollback(&a, &t, &id, &version).await?,
    ))
}

/// Calls a port the app consumes; the platform chooses the installed provider.
pub async fn port(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((id, port)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    Ok(Json(json!(app_ports::call(&a, &h, &id, &port, &v).await?)))
}
