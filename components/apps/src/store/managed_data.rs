//! Shared managed app records: typed writes, optimistic revisions, bounded reads and local RLS context.
use crate::domain::{core_extensions, field_values, ontology, persistence};
use crate::ports::{Platform, RecordRules};
use crate::service::configuration_runtime::validate_fee;
use crate::store::asset_references::references;
use crate::store::core_references::validate_references;
use crate::store::languages::validate_languages;
use crate::store::package_fence::lock_package_write;
use crate::store::{locks, record_write, records};
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::store::history_context::context;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};

/// Keyset pages and indexed equality filters never hydrate an app's entire table.
pub async fn list_page(
    a: &State,
    t: &str,
    m: &Manifest,
    e: &Entity,
    input: &Value,
) -> Result<Value> {
    list_scoped(a, t, m, e, input, None).await
}
/// One bounded page of an entity; a public reader sees only records of its sales channel.
pub async fn list_scoped(
    a: &State,
    t: &str,
    m: &Manifest,
    e: &Entity,
    input: &Value,
    public_channel: Option<&str>,
) -> Result<Value> {
    if !m.permissions.contains(&"data.read".into()) {
        return Err(Error(StatusCode::FORBIDDEN, "App needs data.read".into()));
    }
    let plan = veyndo_app_storage::record_query::prepare(&m.permissions, e, input, public_channel)
        .map_err(persistence::error)?;
    let mut tx = a.db.begin().await?;
    locks::tenant_scope(&mut tx, t).await?;
    let page = veyndo_app_storage::queries::list(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: t,
            app: &m.id,
        },
        &e.name,
        &plan.query(),
    )
    .await
    .map_err(persistence::error)?;
    tx.commit().await?;
    let mut result = veyndo_app_storage::record_query::response(e, plan.limit, page);
    if let Some(graph) = ontology::project(t, m, e, &result["elements"]) {
        result["ontology"] = graph;
    }
    Ok(result)
}
/// A proposal binds one indexed record, including records beyond the first page.
pub async fn record_revision(
    a: &State,
    t: &str,
    m: &Manifest,
    e: &Entity,
    id: &str,
) -> Result<i64> {
    if !m.permissions.contains(&"data.read".into()) {
        return Err(Error(StatusCode::FORBIDDEN, "App needs data.read".into()));
    }
    let mut tx = a.db.begin().await?;
    locks::tenant_scope(&mut tx, t).await?;
    let revision = records::get(&mut tx, t, &m.id, &e.name, id, false)
        .await?
        .and_then(|row| row["revision"].as_i64());
    tx.commit().await?;
    Ok(revision.unwrap_or(0))
}
/// Saves one record for an actor with the rights of its entity and records who did it.
pub async fn save(
    a: &Platform,
    t: &str,
    m: &Manifest,
    e: &Entity,
    v: &Value,
    h: &RequestContext,
) -> Result<Value> {
    core_extensions::access(e, h, true)?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, h, "app.save").await?;
    let result = save_tx(a.rules, &mut tx, t, m, e, v).await?;
    tx.commit().await?;
    Ok(result)
}
/// Saves one record inside the transaction of the caller.
pub async fn save_tx(
    rules: &dyn RecordRules,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
    e: &Entity,
    v: &Value,
) -> Result<Value> {
    write_tx(rules, tx, t, m, e, v, false).await
}
/// Replayable native imports preserve the ordinary package -> route -> record lock order and all field validators.
pub async fn import_tx(
    rules: &dyn RecordRules,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
    e: &Entity,
    v: &Value,
) -> Result<Value> {
    write_tx(rules, tx, t, m, e, v, true).await
}
async fn write_tx(
    rules: &dyn RecordRules,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
    e: &Entity,
    v: &Value,
    replay: bool,
) -> Result<Value> {
    if !m.permissions.contains(&"data.write".into()) {
        return Err(Error(StatusCode::FORBIDDEN, "App needs data.write".into()));
    }
    lock_package_write(tx, t, m).await?;
    let normalized = rules.normalize(m, v)?;
    let v = &normalized;
    field_values::validate(e, &v["fields"])?;
    rules.validate(tx, t, m, e, v).await?;
    validate_references(tx, t, e, &v["fields"]).await?;
    references(tx, t, e, &v["fields"]).await?;
    if e.fields
        .iter()
        .any(|f| f.translatable || f.kind == "richtext")
    {
        validate_languages(tx, t, e, &v["fields"]).await?;
    }
    if let Some(c) = &m.configuration
        && c.entity == e.name
        && let Some(fee) = v["fields"][&c.price_field].as_i64()
    {
        validate_fee(c, fee).await?;
    }

    let id = v["id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("Record id required"))?;
    locks::tenant_scope(tx, t).await?;
    record_write::apply(tx, t, m, e, id, v, replay).await
}
