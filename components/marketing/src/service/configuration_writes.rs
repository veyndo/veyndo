//! Typed configuration reads and revision-bound saves of rules, promotions, flows and channels; flow drafts validate without saving.
use crate::domain::configuration_kinds::{configuration_right, table};
use crate::domain::flow_actions::permission;
use crate::domain::flow_text;
use crate::domain::pipeline::Node;
use crate::domain::rules::Condition;
use crate::http::flow_jobs::values as job_values;
use crate::ports::ConfigurationHost;
use crate::store::channel_config::validate_channel_config;
use crate::store::configuration_lifecycle::{lock, validate_references};
use crate::store::configuration_writes as rows;
use crate::types::channel::normalize_channel;
use crate::types::flow::Flow;
use crate::types::promotion::{Promotion, preserve_import, validate_promotion};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::{Raw, lenient, merge, view};
use veyndo_core::shop_scope::header;
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, State, bad, conflict, store::history_context};
use veyndo_identity::domain::permissions::allowed;
use veyndo_identity::{assistant_access, merchant, permit};
use veyndo_shop::domain::defaults::decode;
use veyndo_shop::service::read_cache::config;

/// Capability that validates a flow draft without saving it.
pub const DRAFT_CAPABILITY: (&str, &str) = (
    "automation.validate_flow",
    "Validate a Flow draft and expected revision without saving or queuing it",
);

/// Every definition the caller may read, with the flow execution summaries.
pub async fn list(a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    if !["automation.read", "channels.read"]
        .iter()
        .any(|s| allowed(h, s))
    {
        permit(h, "automation.read")?;
    }
    let mut result = serde_json::Map::new();
    for kind in ["rules", "promotions", "flows", "channels"] {
        if !allowed(h, configuration_right(kind, false)?) {
            result.insert(kind.into(), json!([]));
            continue;
        }
        let rows = rows::list(a, &t, kind).await?;
        result.insert(
            kind.into(),
            json!(
                rows.into_iter()
                    .map(|r| json!({"id":r.id,"data":r.data,"revision":r.revision}))
                    .collect::<Vec<_>>()
            ),
        );
    }
    let settings = config(a, &t).await?.0;
    if let Some(channels) = result.get_mut("channels").and_then(Value::as_array_mut) {
        for channel in channels
            .iter_mut()
            .filter(|c| view::<Identified>(c).id == "default")
        {
            // A channel row is an object; its document slot is created when missing, as indexing did.
            if let Some(row) = channel.as_object_mut() {
                let data = row.entry("data").or_insert(Value::Null);
                merge(data, json!({ "locales": settings.locales }))?;
            }
        }
    }
    let jobs = if allowed(h, "automation.read") {
        job_values(a, &t, h).await?
    } else {
        json!([])
    };
    result.insert("jobs".into(), jobs);
    Ok(Value::Object(result))
}

/// Records the publishing actor; queued work rechecks that actor's current rights.
pub fn bind_actor(h: &RequestContext, data: &mut Value) -> Result<()> {
    data.as_object_mut().ok_or(bad("Invalid flow"))?.insert(
        "actor".into(),
        json!(header(h, "x-rac-user").unwrap_or("bootstrap")),
    );
    Ok(())
}

/// The publishing actor holds the native rights of every action the flow runs.
pub fn authorize_flow(h: &RequestContext, f: &Flow) -> Result<()> {
    fn action(h: &RequestContext, name: &str) -> Result<()> {
        permit(h, permission(name))?;
        if matches!(name, "ai_proposal" | "work_proposal") {
            assistant_access(h)?;
        }
        if matches!(name, "ai_proposal" | "work_proposal" | "knowledge.extract") {
            permit(h, "knowledge.read")?;
        }
        Ok(())
    }
    if let Some(p) = &f.pipeline {
        for node in &p.nodes {
            if let Node::Action { action: name, .. } = node {
                action(h, name)?;
            }
        }
    } else {
        action(h, &f.action)?;
    }
    Ok(())
}

/// The JSON fields `validate_draft` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateDraftInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    data: Value,
}

/// Validates a flow draft at its expected revision without saving or queuing it.
pub async fn validate_draft<H: ConfigurationHost>(
    host: &H,
    h: &RequestContext,
    id: &str,
    v: &Value,
) -> Result<Value> {
    let input: ValidateDraftInput = view(v);
    permit(h, "automation.manage")?;
    let t = merchant(h)?;
    if !identifier(id) {
        return Err(bad("Invalid configuration ID"));
    }
    let revision = input.revision.ok_or(bad("Revision required"))?;
    let mut data = input.data.clone();
    bind_actor(h, &mut data)?;
    preflight(host, &t, h, (id, revision), &data).await
}

/// Authoring checks see one consistent snapshot and acquire no publication locks.
async fn preflight<H: ConfigurationHost>(
    host: &H,
    t: &str,
    h: &RequestContext,
    (id, expected): (&str, i64),
    data: &Value,
) -> Result<Value> {
    let mut tx = host.core().db.begin_read_snapshot().await?;
    validate_config(host, &mut tx, (t, h), data, false).await?;
    validate_references(&mut tx, t, "flows", id, data).await?;
    let revision = rows::revision(&mut tx, t, "flows", id, false).await?;
    if revision.unwrap_or(0) != expected {
        return Err(conflict("Configuration revision changed"));
    }
    tx.commit().await?;
    Ok(json!({"valid":true,"saved":false,"revision":expected}))
}

/// Native save and release validate final settings and installed app actions in their write transaction.
pub async fn validate_flow_config<H: ConfigurationHost>(
    host: &H,
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    h: &RequestContext,
    value: &Value,
) -> Result<()> {
    validate_config(host, tx, (t, h), value, true).await
}

/// The JSON fields `validate_config` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateConfigInput {
    name: Value,
}

async fn validate_config<H: ConfigurationHost>(
    host: &H,
    tx: &mut Transaction<'_, Postgres>,
    (t, h): (&str, &RequestContext),
    value: &Value,
    lock: bool,
) -> Result<()> {
    let input: ValidateConfigInput = view(value);
    let f: Flow = serde_json::from_value(value.clone()).map_err(|_| bad("Invalid flow"))?;
    f.validate()?;
    authorize_flow(h, &f)?;
    host.validate_template(tx, t, h, &f).await?;
    let settings = decode(rows::settings(tx, t, lock).await?)?;
    settings.languages().validate_names(&input.name, 100)?;
    flow_text::validate(&f, &settings)?;
    host.validate_apps(tx, t, h, &f, lock).await
}

/// A saved commerce rule: its condition and active flag.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SavedRule {
    condition: Value,
    active: Value,
}

/// Kind-specific admission before the write transaction: rule shape, promotion schema and campaign window, channel defaults.
async fn admit(a: &State, kind: &str, id: &str, data: &mut Value, t: &str) -> Result<()> {
    let (settings, _) = config(a, t).await?;
    match kind {
        "rules" => {
            let rule: SavedRule = view(data);
            let c: Condition = serde_json::from_value(rule.condition)
                .map_err(|_| bad("Unsupported rule condition"))?;
            c.validate(0)?;
            if !rule.active.is_boolean() {
                return Err(bad("Rule active flag required"));
            }
        }
        "promotions" => {
            let p: Promotion = serde_json::from_value(data.clone())
                .map_err(|_| bad("Invalid promotion schema"))?;
            validate_promotion(&p)?;
            for date in [&p.start, &p.end] {
                if let Some(date) = date
                    && !rows::timestamp(a, date).await.unwrap_or(false)
                {
                    return Err(bad("Invalid campaign timestamp"));
                }
            }
            if p.start.is_some()
                && p.end.is_some()
                && !rows::ordered(a, p.start.as_deref(), p.end.as_deref()).await?
            {
                return Err(bad("Campaign end precedes start"));
            }
        }
        "flows" => {}
        "channels" => normalize_channel(data, id, &settings)?,
        _ => unreachable!(),
    }
    settings
        .languages()
        .validate_names(&view::<Named>(data).name, 100)
}

/// The localized name every definition carries.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Named {
    name: Value,
}

/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    data: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Saves one definition at its expected revision; `validateOnly` previews a flow draft.
pub async fn save<H: ConfigurationHost>(
    host: &H,
    h: &RequestContext,
    (kind, id): (&str, &str),
    v: &Value,
) -> Result<Value> {
    let input: SaveInput = view(v);
    permit(h, configuration_right(kind, true)?)?;
    let t = merchant(h)?;
    if !identifier(id) {
        return Err(bad("Invalid configuration ID"));
    }
    let table = table(kind)?;
    let mut data = input.data.clone();
    let expected = input.revision.ok_or(bad("Revision required"))?;
    let validate_only = v
        .get("validateOnly")
        .map(|flag| flag.as_bool().ok_or(bad("validateOnly must be a boolean")))
        .transpose()?
        .unwrap_or(false);
    if kind == "flows" {
        bind_actor(h, &mut data)?;
        if validate_only {
            return preflight(host, &t, h, (id, expected), &data).await;
        }
    } else if validate_only {
        return Err(bad("Draft validation is available for flows"));
    }
    let a = host.core();
    admit(a, kind, id, &mut data, &t).await?;
    let mut tx = a.db.begin().await?;
    // Match selective release: automation admission precedes settings and app locks.
    lock(&mut tx, &t).await?;
    if kind == "channels" {
        rows::lock_settings(&mut tx, &t).await?;
        let settings = decode(rows::settings(&mut tx, &t, false).await?)?;
        validate_channel_config(&mut tx, &t, id, &data, &settings).await?;
    }
    if kind == "flows" {
        validate_flow_config(host, &mut tx, &t, h, &data).await?;
    }
    history_context::context(&mut tx, h, "merchant").await?;
    validate_references(&mut tx, &t, kind, id, &data).await?;
    let rev = locked_revision(&mut tx, &t, (kind, table), id).await?;
    if rev.unwrap_or(0) != expected {
        return Err(conflict("Configuration revision changed"));
    }
    if kind == "promotions" {
        admit_import(&mut tx, &t, id, &data).await?;
    }
    write(&mut tx, (&t, kind), id, &data).await?;
    if kind == "channels" {
        host.configure_channel(&mut tx, &t, id, &data, rev.is_none())
            .await?;
    }
    tx.commit().await?;
    Ok(json!({"saved":true,"revision":expected+1}))
}

async fn locked_revision(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    (kind, table): (&str, &str),
    id: &str,
) -> Result<Option<i64>> {
    rows::advisory_lock(tx, &format!("{t}:{table}:{id}")).await?;
    rows::revision(tx, t, kind, id, true).await
}

/// The JSON fields `admit_import` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdmitImportInput {
    source_import: Value,
    #[serde(deserialize_with = "lenient")]
    code: Option<String>,
}

/// Imported provenance stays immutable and an imported coupon code never shadows another native promotion.
async fn admit_import(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    let input: AdmitImportInput = view(data);
    let old = rows::promotion(&mut **tx, t, id).await?;
    preserve_import(&old.unwrap_or(Value::Null), data)?;
    if !input.source_import.is_null() {
        let duplicate = rows::code_taken(tx, t, id, input.code.as_deref()).await?;
        if duplicate {
            return Err(conflict(
                "Coupon code already belongs to another native promotion; review the mapping",
            ));
        }
    }
    Ok(())
}

/// The JSON fields `write` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WriteInput {
    name: Value,
    condition: Value,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
}

async fn write(
    tx: &mut Transaction<'_, Postgres>,
    (t, kind): (&str, &str),
    id: &str,
    data: &Value,
) -> Result<()> {
    let input: WriteInput = view(data);
    if kind == "rules" {
        let rule = rows::RuleColumns {
            name: &input.name,
            condition: &input.condition,
            active: input.active,
        };
        rows::write_rule(tx, t, id, rule).await
    } else {
        rows::write_document(tx, t, kind, id, data).await
    }
}

/// Imported promotions use the ordinary revisioned owner and retain immutable provenance until a merchant reviews activation.
/// An imported coupon: its activation state and its source provenance.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImportedPromotion {
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    source_import: Raw<ImportReview>,
}

/// Whether the merchant reviewed an imported coupon before activation.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImportReview {
    #[serde(deserialize_with = "lenient")]
    activation_reviewed: Option<bool>,
}

pub async fn import_promotion<H: ConfigurationHost>(
    host: &H,
    h: &RequestContext,
    id: &str,
    data: &Value,
) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "automation.manage")?;
    let a = host.core();
    if parent(a, &t).await?.is_some() {
        return Err(bad(
            "Coupon migration needs its live workspace; activation remains separate",
        ));
    }
    let imported: ImportedPromotion = view(data);
    if imported.active != Some(false) || imported.source_import.activation_reviewed != Some(false) {
        return Err(bad("Imported coupons must start inactive and unreviewed"));
    }
    let previous = rows::promotion(&a.db, &t, id).await?;
    if let Some(previous) = previous {
        let incoming: ImportDigest = view(&imported.source_import.raw);
        let stored = view::<StoredImport>(&previous).source_import;
        preserve_import(&previous, data)?;
        if stored.is_null() || view::<ImportDigest>(&stored).record_digest != incoming.record_digest
        {
            return Err(conflict(
                "Imported coupon source changed; review its conflict",
            ));
        }
        return Ok(json!({"id":id,"replayed":true}));
    }
    save(
        host,
        h,
        ("promotions", id),
        &json!({"data":data,"revision":0}),
    )
    .await?;
    Ok(json!({"id":id,"replayed":false,"active":false}))
}

/// The provenance a stored imported promotion keeps.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StoredImport {
    source_import: Value,
}

/// The digest of the source record an imported promotion came from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImportDigest {
    record_digest: Value,
}

/// The id of a listed definition.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Identified {
    id: Value,
}
