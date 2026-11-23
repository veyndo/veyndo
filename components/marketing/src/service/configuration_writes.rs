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
use crate::types::channel::normalize_channel;
use crate::types::flow::Flow;
use crate::types::promotion::{Promotion, preserve_import, validate_promotion};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use veyndo_app_model::declarations::identifier;
use veyndo_core::shop_scope::{header, parent};
use veyndo_core::{RequestContext, Result, State, bad, conflict, history_context};
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
    let mut result = json!({});
    for kind in ["rules", "promotions", "flows", "channels"] {
        if !allowed(h, configuration_right(kind, false)?) {
            result[kind] = json!([]);
            continue;
        }
        let table = table(kind)?;
        let sql = if kind == "rules" {
            format!(
                "SELECT id,jsonb_build_object('name',name,'condition',condition,'active',active) AS data,revision FROM {table} WHERE tenant=$1 ORDER BY id LIMIT 100"
            )
        } else {
            format!("SELECT id,data,revision FROM {table} WHERE tenant=$1 ORDER BY id LIMIT 100")
        };
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(&t)
            .fetch_all(&a.db)
            .await?;
        result[kind]=json!(rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"data":r.get::<Value,_>("data"),"revision":r.get::<i64,_>("revision")})).collect::<Vec<_>>());
    }
    let settings = config(a, &t).await?.0;
    if let Some(channels) = result["channels"].as_array_mut() {
        for channel in channels.iter_mut().filter(|c| c["id"] == "default") {
            channel["data"]["locales"] = json!(settings.locales);
        }
    }
    result["jobs"] = if allowed(h, "automation.read") {
        job_values(a, &t, h).await?
    } else {
        json!([])
    };
    Ok(result)
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

/// Validates a flow draft at its expected revision without saving or queuing it.
pub async fn validate_draft<H: ConfigurationHost>(
    host: &H,
    h: &RequestContext,
    id: &str,
    v: &Value,
) -> Result<Value> {
    permit(h, "automation.manage")?;
    let t = merchant(h)?;
    if !identifier(id) {
        return Err(bad("Invalid configuration ID"));
    }
    let revision = v["revision"].as_i64().ok_or(bad("Revision required"))?;
    let mut data = v["data"].clone();
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
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM commerce_flows WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
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

async fn validate_config<H: ConfigurationHost>(
    host: &H,
    tx: &mut Transaction<'_, Postgres>,
    (t, h): (&str, &RequestContext),
    value: &Value,
    lock: bool,
) -> Result<()> {
    let f: Flow = serde_json::from_value(value.clone()).map_err(|_| bad("Invalid flow"))?;
    f.validate()?;
    authorize_flow(h, &f)?;
    host.validate_template(tx, t, h, &f).await?;
    let settings_query = if lock {
        "SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE"
    } else {
        "SELECT data FROM commerce_settings WHERE tenant=$1"
    };
    let settings = decode(
        sqlx::query_scalar(settings_query)
            .bind(t)
            .fetch_one(&mut **tx)
            .await?,
    )?;
    settings.languages().validate_names(&value["name"], 100)?;
    flow_text::validate(&f, &settings)?;
    host.validate_apps(tx, t, h, &f, lock).await
}

/// Kind-specific admission before the write transaction: rule shape, promotion schema and campaign window, channel defaults.
async fn admit(a: &State, kind: &str, id: &str, data: &mut Value, t: &str) -> Result<()> {
    let (settings, _) = config(a, t).await?;
    match kind {
        "rules" => {
            let c: Condition = serde_json::from_value(data["condition"].clone())
                .map_err(|_| bad("Unsupported rule condition"))?;
            c.validate(0)?;
            if !data["active"].is_boolean() {
                return Err(bad("Rule active flag required"));
            }
        }
        "promotions" => {
            let p: Promotion = serde_json::from_value(data.clone())
                .map_err(|_| bad("Invalid promotion schema"))?;
            validate_promotion(&p)?;
            for date in [&p.start, &p.end] {
                if let Some(date) = date
                    && !sqlx::query_scalar::<_, bool>("SELECT $1::timestamptz IS NOT NULL")
                        .bind(date)
                        .fetch_one(&a.db)
                        .await
                        .unwrap_or(false)
                {
                    return Err(bad("Invalid campaign timestamp"));
                }
            }
            if p.start.is_some()
                && p.end.is_some()
                && !sqlx::query_scalar::<_, bool>("SELECT $1::timestamptz <= $2::timestamptz")
                    .bind(&p.start)
                    .bind(&p.end)
                    .fetch_one(&a.db)
                    .await?
            {
                return Err(bad("Campaign end precedes start"));
            }
        }
        "flows" => {}
        "channels" => normalize_channel(data, id, &settings),
        _ => unreachable!(),
    }
    settings.languages().validate_names(&data["name"], 100)
}

/// Saves one definition at its expected revision; `validateOnly` previews a flow draft.
pub async fn save<H: ConfigurationHost>(
    host: &H,
    h: &RequestContext,
    (kind, id): (&str, &str),
    v: &Value,
) -> Result<Value> {
    permit(h, configuration_right(kind, true)?)?;
    let t = merchant(h)?;
    if !identifier(id) {
        return Err(bad("Invalid configuration ID"));
    }
    let table = table(kind)?;
    let mut data = v["data"].clone();
    let expected = v["revision"].as_i64().ok_or(bad("Revision required"))?;
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
        sqlx::query("SELECT revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
        let settings = decode(
            sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1")
                .bind(&t)
                .fetch_one(&mut *tx)
                .await?,
        )?;
        validate_channel_config(&mut tx, &t, id, &data, &settings).await?;
    }
    if kind == "flows" {
        validate_flow_config(host, &mut tx, &t, h, &data).await?;
    }
    history_context::context(&mut tx, h, "merchant").await?;
    validate_references(&mut tx, &t, kind, id, &data).await?;
    let rev = locked_revision(&mut tx, &t, table, id).await?;
    if rev.unwrap_or(0) != expected {
        return Err(conflict("Configuration revision changed"));
    }
    if kind == "promotions" {
        admit_import(&mut tx, &t, id, &data).await?;
    }
    write(&mut tx, (&t, table), id, &data).await?;
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
    table: &str,
    id: &str,
) -> Result<Option<i64>> {
    let sql = format!("SELECT revision FROM {table} WHERE tenant=$1 AND id=$2 FOR UPDATE");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,17))")
        .bind(format!("{t}:{table}:{id}"))
        .execute(&mut **tx)
        .await?;
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(t)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?)
}

/// Imported provenance stays immutable and an imported coupon code never shadows another native promotion.
async fn admit_import(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    let old: Option<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_promotions WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    preserve_import(&old.unwrap_or(Value::Null), data)?;
    if !data["sourceImport"].is_null() {
        let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commerce_promotions WHERE tenant=$1 AND id<>$2 AND data->>'code'=$3)").bind(t).bind(id).bind(data["code"].as_str()).fetch_one(&mut **tx).await?;
        if duplicate {
            return Err(conflict(
                "Coupon code already belongs to another native promotion; review the mapping",
            ));
        }
    }
    Ok(())
}

async fn write(
    tx: &mut Transaction<'_, Postgres>,
    (t, table): (&str, &str),
    id: &str,
    data: &Value,
) -> Result<()> {
    if table == "commerce_rules" {
        sqlx::query("INSERT INTO commerce_rules(tenant,id,name,condition,active) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET name=EXCLUDED.name,condition=EXCLUDED.condition,active=EXCLUDED.active,revision=commerce_rules.revision+1").bind(t).bind(id).bind(&data["name"]).bind(&data["condition"]).bind(data["active"].as_bool()).execute(&mut **tx).await?;
    } else {
        let sql = format!(
            "INSERT INTO {table}(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision={table}.revision+1"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(t)
            .bind(id)
            .bind(data)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Imported promotions use the ordinary revisioned owner and retain immutable provenance until a merchant reviews activation.
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
    if data["active"] != false || data["sourceImport"]["activationReviewed"] != false {
        return Err(bad("Imported coupons must start inactive and unreviewed"));
    }
    let previous: Option<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_promotions WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?;
    if let Some(previous) = previous {
        let incoming = &data["sourceImport"];
        let stored = &previous["sourceImport"];
        preserve_import(&previous, data)?;
        if stored.is_null() || stored["recordDigest"] != incoming["recordDigest"] {
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
