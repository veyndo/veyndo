//! Registered managed app actions join the same preview/approve transaction as core changes; planners see current authorized models only.
use crate::domain::action_admission::authorized;
use crate::domain::discovery::relevant;
use crate::domain::planning::{AppChange, authorize_planned_action, bounded_context};
use crate::domain::{core_extensions, field_values};
use crate::ports::{Platform, RecordRules};
use crate::store::managed_data;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::shop_scope::channel;
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::permit;

/// The installed apps a planner may use for a request: their selected tools and a bounded sample of records.
pub async fn planning_context(
    a: &State,
    t: &str,
    h: &RequestContext,
    query: &str,
) -> Result<Value> {
    let rows = crate::store::packages::active_manifest_values(a, t).await?;
    let mut manifests = rows
        .into_iter()
        .map(|r| serde_json::from_value::<Manifest>(r).map_err(|_| bad("Invalid app package")))
        .collect::<Result<Vec<_>>>()?;
    manifests.retain(|m| {
        m.actions.iter().any(|act| {
            authorized(h, act)
                && act.mcp != Some(false)
                && m.intelligence
                    .as_ref()
                    .is_none_or(|ai| ai.tools.contains(&act.name))
        })
    });
    manifests.sort_by_key(|m| (std::cmp::Reverse(relevant(m, query)), m.id.clone()));
    let mut context = vec![];
    for m in manifests.into_iter().take(8) {
        let mut records = vec![];
        if m.permissions.contains(&"data.read".into()) {
            for e in m
                .entities
                .iter()
                .filter(|e| {
                    m.actions.iter().any(|act| {
                        act.handler == "list"
                            && act.mcp != Some(false)
                            && act.entity.as_deref() == Some(e.name.as_str())
                            && authorized(h, act)
                            && m.intelligence
                                .as_ref()
                                .is_none_or(|ai| ai.tools.contains(&act.name))
                    }) && m
                        .intelligence
                        .as_ref()
                        .is_none_or(|ai| ai.entities.contains(&e.name))
                })
                .take(4)
            {
                let reader = m
                    .actions
                    .iter()
                    .find(|act| {
                        act.handler == "list"
                            && act.mcp != Some(false)
                            && act.entity.as_deref() == Some(e.name.as_str())
                            && authorized(h, act)
                            && m.intelligence
                                .as_ref()
                                .is_none_or(|ai| ai.tools.contains(&act.name))
                    })
                    .ok_or(bad("No authorized app reader"))?;
                let mut data = managed_data::list_scoped(
                    a,
                    t,
                    &m,
                    e,
                    &json!({"limit":12}),
                    reader.public.then(|| channel(h)),
                )
                .await?;
                data["elements"] = json!(
                    data["elements"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .take(12)
                        .collect::<Vec<_>>()
                );
                if let Some(rows) = data["elements"].as_array_mut() {
                    for row in rows {
                        *row = bounded_context(row, 0);
                    }
                }
                records.push(json!({"entity":e.name,"records":data}));
            }
        }
        let actions = m
            .actions
            .iter()
            .filter(|act| {
                authorized(h, act)
                    && act.mcp != Some(false)
                    && m.intelligence
                        .as_ref()
                        .is_none_or(|ai| ai.tools.contains(&act.name))
            })
            .collect::<Vec<_>>();
        let mut intelligence = m.intelligence.clone();
        if let Some(ai) = &mut intelligence {
            ai.tools
                .retain(|name| actions.iter().any(|act| act.name == *name));
            ai.entities
                .retain(|name| records.iter().any(|record| record["entity"] == *name));
            ai.ontology
                .retain(|node| ai.entities.contains(&node.entity));
        }
        let item =
            json!({"app":m.id,"actions":actions,"records":records,"intelligence":intelligence});
        if context
            .iter()
            .map(|v: &Value| v.to_string().len())
            .sum::<usize>()
            + item.to_string().len()
            > 32768
        {
            break;
        }
        context.push(item);
    }
    Ok(json!(context))
}
/// Binds a proposed save to the current package and record revision after checking its shape and rights.
pub async fn bind_change(
    a: &Platform,
    t: &str,
    c: &mut AppChange,
    h: &RequestContext,
) -> Result<()> {
    if c.arguments_json.len() > 32768 {
        return Err(bad("App change too large"));
    }
    let row = sqlx::query!(
        "SELECT manifest,revision FROM app_packages WHERE tenant=$1 AND id=$2 AND active",
        t,
        c.app
    )
    .fetch_optional(&a.db)
    .await?
    .ok_or(bad("Unknown active app in proposal"))?;
    let m: Manifest =
        serde_json::from_value(row.manifest.clone()).map_err(|_| bad("Invalid app package"))?;
    let action = m
        .actions
        .iter()
        .find(|a| a.name == c.action && a.handler == "save")
        .ok_or(bad("Planner supports managed save actions only"))?;
    authorize_planned_action(&m, action, h)?;
    let e = m
        .entities
        .iter()
        .find(|e| Some(&e.name) == action.entity.as_ref())
        .ok_or(bad("Unknown app entity"))?;
    let mut args: Value =
        serde_json::from_str(&c.arguments_json).map_err(|_| bad("Invalid app arguments JSON"))?;
    field_values::validate(e, &args["fields"])?;
    a.rules.check(&m, e, &args["fields"])?;
    let id = args["id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("App record ID required"))?;
    let revision = managed_data::record_revision(a, t, &m, e, id).await?;
    args["revision"] = json!(revision);
    c.arguments_json = args.to_string();
    c.expected_app_revision = row.revision;
    Ok(())
}
/// Applies an approved proposal through the ordinary managed save, if the package is unchanged.
pub async fn apply_change(
    rules: &dyn RecordRules,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    c: &AppChange,
    h: &RequestContext,
) -> Result<()> {
    let row = sqlx::query!(
        "SELECT manifest,revision FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
        t,
        c.app
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(conflict("App is not active"))?;
    if row.revision != c.expected_app_revision {
        return Err(conflict("App changed since proposal"));
    }
    let m: Manifest =
        serde_json::from_value(row.manifest.clone()).map_err(|_| bad("Invalid app package"))?;
    let action = m
        .actions
        .iter()
        .find(|a| a.name == c.action && a.handler == "save")
        .ok_or(bad("Unknown managed app action"))?;
    authorize_planned_action(&m, action, h)?;
    let e = m
        .entities
        .iter()
        .find(|e| Some(&e.name) == action.entity.as_ref())
        .ok_or(bad("Unknown app entity"))?;
    let args: Value =
        serde_json::from_str(&c.arguments_json).map_err(|_| bad("Invalid app arguments"))?;
    managed_data::save_tx(rules, tx, t, &m, e, &args).await?;
    Ok(())
}

/// The managed-save models of the selected apps that the actor may write, for a planner.
pub async fn contracts(a: &Platform, t: &str, h: &RequestContext, ids: &[String]) -> Result<Value> {
    if ids.is_empty() {
        return Ok(json!([]));
    }
    permit(h, "apps.read")?;
    if ids.len() > 12 || ids.iter().any(|id| !identifier(id)) {
        return Err(bad("App contract selection exceeds the planning limit"));
    }
    let rows = sqlx::query!(
        "SELECT manifest,revision FROM app_packages WHERE tenant=$1 AND id=ANY($2) AND active ORDER BY id",
        t,
        ids
    )
    .fetch_all(&a.db)
    .await?;
    let mut result = Vec::new();
    for row in rows {
        let m: Manifest =
            serde_json::from_value(row.manifest.clone()).map_err(|_| bad("Invalid app package"))?;
        for action in m.actions.iter().filter(|action| action.handler == "save") {
            if authorize_planned_action(&m, action, h).is_err() {
                continue;
            }
            let e = m
                .entities
                .iter()
                .find(|e| Some(&e.name) == action.entity.as_ref())
                .ok_or(bad("Unknown app entity"))?;
            if core_extensions::access(e, h, true).is_err() {
                continue;
            }
            let entry = json!({"app":m.id,"version":m.version,"revision":row.revision,"action":action.name,"inputSchema":action.input_schema,"entity":e.name,"fields":e.fields,"nestedFormat":a.rules.planning_format(&m)});
            if json!(result).to_string().len() + entry.to_string().len() > 32768 {
                return Err(bad(
                    "App contracts exceed the planning context; select fewer apps",
                ));
            }
            result.push(entry);
        }
    }
    Ok(json!(result))
}
