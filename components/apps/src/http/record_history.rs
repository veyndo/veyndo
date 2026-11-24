//! Bounded app-record history, current entity rights and restoration through the normal managed save path.
use crate::domain::entity_access::entity_access;
use crate::ports::Platform;
use crate::store::packages::package;
use crate::store::{record_restore, records};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::record_history as shared;
use veyndo_core::document::{lenient, view};
use veyndo_core::history_context::context;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::{merchant, permit};

fn entity<'a>(m: &'a Manifest, name: &str, h: &RequestContext, write: bool) -> Result<&'a Entity> {
    let e = m
        .entities
        .iter()
        .find(|e| e.name == name)
        .ok_or(bad("Unknown app entity"))?;
    permit(h, if write { "apps.manage" } else { "apps.read" })?;
    permit(h, "catalog.read")?;
    entity_access(m, e, h, "list")?;
    if write {
        if !m
            .actions
            .iter()
            .any(|a| a.entity.as_deref() == Some(e.name.as_str()) && a.handler == "save")
        {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "This entity has no restorable save action".into(),
            ));
        }
        entity_access(m, e, h, "save")?;
    }
    Ok(e)
}
/// The stored versions of one record, with the fields the reader may see today.
pub async fn list(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((app, name, id)): Path<(String, String, String)>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &app, true).await?;
    let e = entity(&m, &name, &h, false)?;
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT set_config('rac.tenant',$1,true)")
        .bind(&t)
        .execute(&mut *tx)
        .await?;
    let rows = shared::list(
        &mut tx,
        records::key(&t, &app, &name, &id),
        &record_restore::Current(&h),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"versions":rows.iter().map(|r|json!({"revision":r.get::<i64,_>("revision"),"fields":e.fields.iter().map(|f|(f.name.clone(),r.get::<Value,_>("fields")[&f.name].clone())).collect::<serde_json::Map<String,Value>>(),"actor":r.get::<Option<String>,_>("actor"),"source":r.get::<String,_>("source"),"createdAt":r.get::<String,_>("created_at")})).collect::<Vec<_>>(),"canRestore":entity(&m,&e.name,&h,true).is_ok()}),
    ))
}
/// The JSON fields `restore` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RestoreInput {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    revision: Value,
}

/// Restores a stored version through the ordinary save of the entity, after explicit approval.
pub async fn restore(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((app, name, id, revision)): Path<(String, String, String, i64)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: RestoreInput = view(&v);
    let t = merchant(&h)?;
    let m = package(&a, &t, &app, true).await?;
    let e = entity(&m, &name, &h, true)?;
    if input.approve != Some(true) || input.revision.as_i64().is_none() {
        return Err(bad("Approve restoration with the current revision"));
    }
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "app.history.restore").await?;
    sqlx::query("SELECT set_config('rac.tenant',$1,true)")
        .bind(&t)
        .execute(&mut *tx)
        .await?;
    let result = shared::restore(
        &mut tx,
        records::key(&t, &app, &name, &id),
        revision,
        &record_restore::Restore {
            rules: a.rules,
            actor: &h,
            manifest: &m,
            entity: e,
            revision: &input.revision,
        },
    )
    .await
    .map_err(record_restore::error)?;
    tx.commit().await?;
    Ok(Json(result))
}

/// MCP calls the exact HTTP domain operations, retaining entity and historical read permissions.
pub fn schema(name: &str) -> Option<Value> {
    if !["merchant.app.history", "merchant.app.restore"].contains(&name) {
        return None;
    }
    Some(
        json!({"type":"object","additionalProperties":false,"properties":{"app":{"type":"string"},"entity":{"type":"string"},"id":{"type":"string"},"version":{"type":"integer"},"revision":{"type":"integer"},"approve":{"type":"boolean"}},"required":if name.ends_with("restore"){vec!["app","entity","id","version","revision","approve"]}else{vec!["app","entity","id"]}}),
    )
}
/// The app record and version a history capability names.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RecordHistoryArgs {
    #[serde(deserialize_with = "lenient")]
    app: Option<String>,
    #[serde(deserialize_with = "lenient")]
    entity: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    version: Option<i64>,
}

/// Runs a history capability through the same operations as the HTTP routes.
pub async fn invoke(a: &Platform, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let input: RecordHistoryArgs = view(v);
    let app = input.app.as_deref().ok_or(bad("App required"))?.to_owned();
    let entity = input
        .entity
        .as_deref()
        .ok_or(bad("Entity required"))?
        .to_owned();
    let id = input
        .id
        .as_deref()
        .ok_or(bad("Record required"))?
        .to_owned();
    if name == "merchant.app.history" {
        let Json(value) = list(Shared(a.clone()), h.clone(), Path((app, entity, id))).await?;
        return Ok(value);
    }
    if name != "merchant.app.restore" {
        return Err(bad("Unknown app history operation"));
    }
    let Json(value) = restore(
        Shared(a.clone()),
        h.clone(),
        Path((
            app,
            entity,
            id,
            input.version.ok_or(bad("Version required"))?,
        )),
        Json(v.clone()),
    )
    .await?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::schema;

    #[test]
    fn only_the_two_history_capabilities_have_a_schema_and_restore_needs_approval() {
        assert!(schema("merchant.app.other").is_none());
        let history = schema("merchant.app.history").unwrap();
        assert_eq!(history["required"].as_array().unwrap().len(), 3);
        let restore = schema("merchant.app.restore").unwrap();
        let required = restore["required"].as_array().unwrap();
        assert!(required.iter().any(|x| x == "approve"));
        assert!(required.iter().any(|x| x == "revision"));
        assert_eq!(restore["additionalProperties"], false);
    }
}
