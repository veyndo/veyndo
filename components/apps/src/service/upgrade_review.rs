//! Probe the actual transactional installer and roll it back; return admission metadata, never app record contents.
use crate::domain::schema_changes;
use crate::ports::Platform;
use crate::store::package_install::install_tx;
use crate::store::{locks, packages};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::store::environments::owned_stage;
use veyndo_core::{Result, bad};

/// What installing a package would change, probed with the real installer and rolled back.
pub async fn inspect(
    a: &Platform,
    live: &str,
    environment: Option<&str>,
    m: &Manifest,
) -> Result<(Option<Manifest>, Value)> {
    let mut tx = a.db.begin_privileged().await?;
    locks::review_timeouts(&mut tx).await?;
    // Child ownership is system metadata: retain the exact parent/private-stage guard in this bounded transaction.
    let t = if let Some(environment) = environment {
        owned_stage(&mut *tx, live, environment).await?;
        environment
    } else {
        live
    };
    a.commerce.lock(&mut tx, t).await?;
    let previous: Option<Value> = packages::manifest_value(&mut tx, t, &m.id).await?;
    let old = previous
        .as_ref()
        .map(|v| serde_json::from_value::<Manifest>(v.clone()))
        .transpose()
        .map_err(|_| bad("Invalid prior app package"))?;
    let migration = old.as_ref().and_then(|old| {
        m.schema_migrations
            .iter()
            .find(|p| p.from_version == old.version)
    });
    let required = old.as_ref().is_some_and(|old| {
        schema_changes::shape(old, m, None).is_err()
            || old.entities.iter().any(|before| {
                m.entities
                    .iter()
                    .find(|e| e.name == before.name)
                    .is_some_and(|after| {
                        after
                            .fields
                            .iter()
                            .any(|f| f.required && !before.fields.iter().any(|b| b.name == f.name))
                    })
            })
    });
    let rows = packages::model_records(&mut tx, t, &m.id).await?;
    let models = rows
        .iter()
        .map(|r| json!({"name":r.entity,"records":r.records}))
        .collect::<Vec<_>>();
    // This owner only writes through this transaction. It does not send events or execute services.
    let result: Result<()> = async {
        install_tx(a.rules, a.commerce, &mut tx, t, m.clone()).await?;
        // The real commit also checks deferred relational constraints; rollback alone would miss them.
        locks::constraints_immediate(&mut tx).await?;
        Ok(())
    }
    .await;
    tx.rollback().await?;
    let issue = match result {
        Ok(_) => Value::Null,
        Err(e) if e.0.is_server_error() => return Err(e),
        Err(e) => {
            let mut p = e.problem();
            p["detail"] = json!(e.1);
            p
        }
    };
    let report = json!({"ready":issue.is_null(),"migrationRequired":required,"migrationSteps":migration.map_or(0,|p|p.steps.len()),"models":models,"issue":issue});
    Ok((old, report))
}
