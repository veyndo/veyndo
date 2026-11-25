//! Reviewed job records use current creator rights and a locked live package; private targets never become app credentials.
use super::{Manifest, approval, credentials, gateway, input_schema, lock_package_write};
use crate::auth;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_app_platform::store::app_host_reads;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{Raw, lenient, view};
use veyndo_core::hash;
use veyndo_core::request_context::RequestContext;
// Retain a connection for the native owner even on the supported two-connection pool.
// This is process admission, not a claim of fleet-wide throughput isolation.
static IMPORT_WRITE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
/// The write and read capability an import of this kind needs.
fn capabilities(kind: &str) -> Result<(&'static str, &'static str)> {
    let capability = match kind {
        "products" | "categories" => "products.write",
        "assets" => "assets.write",
        "reviews" => "reviews.write",
        "coupons" => "promotions.write",
        "pages" => "data.write",
        "customers" => "customers.write",
        "orders" => "orders.import",
        _ => return Err(bad("Native job import domain unavailable")),
    };
    let read_capability = match kind {
        "products" | "categories" | "reviews" => "products.read",
        "assets" => "assets.read",
        "pages" => "data.read",
        "customers" => "customers.read",
        "orders" => "orders.read",
        "coupons" => "promotions.write",
        _ => return Err(bad("Native job import domain unavailable")),
    };
    Ok((capability, read_capability))
}
/// The package must have been approved for the import and the caller must hold its rights.
fn admit_app(
    h: &RequestContext,
    m: &Manifest,
    kind: &str,
    (capability, read_capability): (&str, &str),
    inspect: bool,
) -> Result<()> {
    for p in ["jobs.write", capability]
        .into_iter()
        .chain(inspect.then_some(read_capability))
    {
        if !m.permissions.iter().any(|s| s == p) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Import capability was not approved".into(),
            ));
        }
        credentials::permit(h, p)?;
        auth::permit(h, credentials::scope(p).unwrap())?;
    }
    if ["customers", "orders", "reviews"].contains(&kind) {
        if !m.permissions.iter().any(|p| p == "customers.pii") {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Historical personal data needs explicit PII consent".into(),
            ));
        }
        credentials::permit(h, "customers.pii")?;
        auth::permit(h, "customers.pii")?;
    }
    Ok(())
}
/// The native import input: the reviewed record with exactly the acknowledged source warnings.
/// An import job service's callback: the job, its lease and the source record it hands over.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportCallback {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    lease: Option<String>,
    #[serde(deserialize_with = "lenient")]
    record: Raw<CallbackRecord>,
}

/// The fields of a handed-over source record this callback checks.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CallbackRecord {
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    warnings: Option<Vec<Value>>,
}

/// The reviewed import intent: the options the merchant approved.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportIntent {
    #[serde(deserialize_with = "lenient")]
    options: Raw<ApprovedOptions>,
}

/// The source limitations the merchant acknowledged.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ApprovedOptions {
    #[serde(deserialize_with = "lenient")]
    acknowledged_warnings: Option<Vec<Value>>,
}

fn import_input(v: &Value, intent: &Value, inspect: bool) -> Result<Value> {
    let callback: ImportCallback = view(v);
    let intent: ImportIntent = view(intent);
    let mut input =
        json!({"approve":true,"record":callback.record.raw,"options":intent.options.raw});
    if let Some(approved) = intent.options.acknowledged_warnings.as_ref() {
        let warnings = callback
            .record
            .warnings
            .as_ref()
            .ok_or(bad("Invalid source warnings"))?;
        if approved.len() > 20
            || approved
                .iter()
                .any(|value| value.as_str().is_none_or(|s| s.len() > 2000))
            || warnings.iter().any(|warning| !approved.contains(warning))
        {
            return Err(bad(
                "Review every source limitation before importing this record",
            ));
        }
        if warnings.is_empty() {
            input["options"]
                .as_object_mut()
                .unwrap()
                .remove("acknowledgedWarnings");
        } else {
            input["options"]["acknowledgedWarnings"] = json!(warnings);
        }
    }
    if let Some(chunks) = v.get("assetChunks") {
        input["assetChunks"] = chunks.clone();
    }
    if inspect {
        input.as_object_mut().unwrap().remove("approve");
        input.as_object_mut().unwrap().remove("assetChunks");
    }
    Ok(input)
}
pub(super) async fn execute(
    a: &App,
    h: &RequestContext,
    m: &Manifest,
    v: &Value,
    inspect: bool,
) -> Result<Value> {
    let mut metadata = v.clone();
    metadata
        .as_object_mut()
        .ok_or(bad("Import object required"))?
        .remove("assetChunks");
    input_schema::validate_native(
        &json!({"type":"object","properties":{"id":{"type":"string","maxLength":100},"lease":{"type":"string","maxLength":100},"record":{"type":"object"}},"required":["id","lease","record"],"additionalProperties":false}),
        &metadata,
    )?;
    let live = merchant(a, h)?;
    if veyndo_core::store::environments::parent(a, &live)
        .await?
        .is_some()
    {
        return Err(bad("Job callback requires its live workspace"));
    }
    let callback: ImportCallback = view(v);
    let kind = callback
        .record
        .kind
        .as_deref()
        .ok_or(bad("Import kind required"))?;
    let (capability, read_capability) = capabilities(kind)?;
    admit_app(h, m, kind, (capability, read_capability), inspect)?;
    let _reserve = tokio::time::timeout(std::time::Duration::from_secs(10), IMPORT_WRITE.acquire())
        .await
        .map_err(|_| {
            Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Native import capacity is busy; retry the reviewed job".into(),
            )
        })?
        .map_err(|_| bad("Native import admission unavailable"))?;
    // The native coordinator must read and lock an admitted child environment.
    // This bounded privileged transaction never leaves Core; every query binds live tenant/job/actor.
    let mut tx = a.db.begin_privileged().await?;
    let (actor, action_name, package_digest, intent) = app_host_reads::claimed_job(
        &mut tx,
        &live,
        (&m.id, callback.id.as_deref(), callback.lease.as_deref()),
    )
    .await?;
    if package_digest != approval::canonical_digest(m) {
        return Err(conflict("Reviewed job package changed"));
    }
    let (role, rights) = app_host_reads::creator_grants(&mut tx, &live, &actor).await?;
    let mut identity = RequestContext::new();
    identity.principal.user = Some(actor.clone());
    identity.principal.role = Some(role);
    identity.principal.permissions = (!rights.is_null()).then(|| rights.to_string());
    auth::permit(&identity, "apps.manage")?;
    auth::permit(&identity, credentials::scope(capability).unwrap())?;
    if inspect {
        auth::permit(&identity, credentials::scope(read_capability).unwrap())?;
    }
    if ["customers", "orders", "reviews"].contains(&kind) {
        auth::permit(&identity, "customers.pii")?;
    }
    let action = m
        .actions
        .iter()
        .find(|x| x.name == action_name && x.handler == "job")
        .ok_or(conflict("Job action removed"))?;
    auth::permit(
        &identity,
        action.permission.as_deref().unwrap_or("apps.manage"),
    )?;
    gateway::additional_permissions(&identity, action)?;
    if intent["operation"] == "reconcile" && !inspect {
        return Err(conflict(
            "Read-only reconciliation does not authorize native writes",
        ));
    }
    if intent["kind"] != kind
        || !intent["recordDigests"].as_array().is_some_and(|ds| {
            !ds.is_empty()
                && ds.len() <= 100
                && ds
                    .iter()
                    .any(|d| *d == hash(&callback.record.raw.to_string()))
        })
    {
        return Err(conflict(
            "Record differs from the explicitly reviewed job snapshot",
        ));
    }
    let target = intent["target"]
        .as_str()
        .ok_or(bad("Reviewed target required"))?;
    if ["products", "categories", "pages", "assets"].contains(&kind) {
        auth::permit(&identity, "environments.manage")?;
        app_host_reads::private_target(&mut tx, target, &live).await?;
    } else if target != live {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Operational history belongs to the reviewed live workspace".into(),
        ));
    }
    lock_package_write(&mut tx, &live, m).await?;
    identity.insert(
        "x-tenant",
        target.parse().map_err(|_| bad("Invalid reviewed target"))?,
    );
    identity.principal.tenant = Some(target.into());
    let input = import_input(v, &intent, inspect)?;
    // Native scope switch is limited to a job-locked target in this same admitted workspace.
    let operation = async {
        if inspect {
            crate::operations::imports::inspect(a, &identity, &input).await
        } else {
            crate::operations::imports::invoke(a, &identity, &input).await
        }
    };
    let result = veyndo::tenant_scope::scoped(
        veyndo::tenant_scope::Scope::Personal {
            tenant: target.into(),
            actor,
        },
        operation,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}
