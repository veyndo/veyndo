//! Private source-record intake reuses an installed app's declared native save action, schema, rights and package fence.
use crate::domain::{action_admission, approval, core_extensions, input_schema};
use crate::ports::Platform;
use crate::store::managed_data;
use crate::store::packages::package;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::{lenient, view};
use veyndo_core::history_context::context;
use veyndo_core::shop_scope::parent;
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::{merchant, permit};

/// Imports one source record through the declared save action of the destination app.
pub async fn import_record(
    a: &Platform,
    h: &RequestContext,
    id: &str,
    destination: &Value,
    expected_digest: &str,
) -> Result<Value> {
    let (m, entity_name, input) = admitted(a, h, id, destination, expected_digest).await?;
    let t = merchant(h)?;
    let entity = m
        .entities
        .iter()
        .find(|e| e.name == entity_name)
        .ok_or(bad("Destination entity unavailable"))?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, h, "migration.record.import").await?;
    let result = managed_data::import_tx(a.rules, &mut tx, &t, &m, entity, &input).await?;
    tx.commit().await?;
    Ok(json!({"id":result["id"],"replayed":result["replayed"]==true}))
}

/// The JSON fields `admitted` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdmittedInput {
    #[serde(deserialize_with = "lenient")]
    app: Option<String>,
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    fields: Value,
}

/// The same destination package, declared save action and native field normalization govern comparison.
async fn admitted(
    a: &Platform,
    h: &RequestContext,
    id: &str,
    destination: &Value,
    expected_digest: &str,
) -> Result<(Manifest, String, Value)> {
    let args: AdmittedInput = view(destination);
    permit(h, "apps.manage")?;
    let t = merchant(h)?;
    if parent(a, &t).await?.is_none() {
        return Err(bad("Managed record imports require a private environment"));
    }
    let app = args.app.as_deref().ok_or(bad("Destination app required"))?;
    let action_name = args
        .action
        .as_deref()
        .ok_or(bad("Destination save action required"))?;
    let m = package(a, &t, app, true).await?;
    if expected_digest != approval::canonical_digest(&m) {
        return Err(conflict(
            "Destination app package changed; review the target again",
        ));
    }
    if m.runtime != "declarative" {
        return Err(bad("Import destination must use managed native records"));
    }
    let action = m
        .actions
        .iter()
        .find(|x| x.name == action_name && x.handler == "save" && !x.public)
        .ok_or(bad("Declared private save action required"))?;
    permit(h, action.permission.as_deref().unwrap_or("catalog"))?;
    action_admission::additional(h, action)?;
    let entity = m
        .entities
        .iter()
        .find(|x| Some(&x.name) == action.entity.as_ref())
        .ok_or(bad("Destination entity unavailable"))?;
    core_extensions::access(entity, h, true)?;
    let input = a
        .rules
        .normalize(&m, &json!({"id":id,"revision":0,"fields":args.fields}))?;
    input_schema::validate_native(&action.input_schema, &input)?;
    let name = entity.name.clone();
    Ok((m, name, input))
}
/// The fields an import would write beside the record that is stored now.
pub async fn inspect_import_record(
    a: &Platform,
    h: &RequestContext,
    id: &str,
    destination: &Value,
    expected_digest: &str,
) -> Result<(Value, Option<Value>)> {
    let (m, entity_name, input) = admitted(a, h, id, destination, expected_digest).await?;
    let t = merchant(h)?;
    let entity = m
        .entities
        .iter()
        .find(|e| e.name == entity_name)
        .ok_or(bad("Destination entity unavailable"))?;
    core_extensions::access(entity, h, false)?;
    let actual =
        managed_data::list_page(a, &t, &m, entity, &json!({"id":id,"limit":1})).await?["elements"]
            .as_array()
            .and_then(|xs| xs.first().cloned());
    let expected: serde_json::Map<String, Value> = entity
        .fields
        .iter()
        .map(|f| (f.name.clone(), input["fields"][&f.name].clone()))
        .collect();
    Ok((json!(expected), actual))
}
