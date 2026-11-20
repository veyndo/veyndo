//! One fixed PostgreSQL record store; every operation retains tenant/app/model identity and original wire values.
use crate::domain::core_extensions;
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, Result, bad, conflict};

/// The identity of one record: shop, app, entity and id.
pub fn key<'a>(
    t: &'a str,
    app: &'a str,
    entity: &'a str,
    id: &'a str,
) -> veyndo_app_storage::RecordKey<'a> {
    veyndo_app_storage::RecordKey {
        scope: veyndo_app_storage::Scope { tenant: t, app },
        entity,
        id,
    }
}
/// The advisory lock name of one record.
pub fn lock_key(t: &str, app: &str, entity: &str, id: &str) -> String {
    veyndo_app_storage::records::lock_key(key(t, app, entity, id))
}
/// A stored row in the wire form of its entity.
pub fn project(e: &Entity, row: &Value) -> Value {
    veyndo_app_storage::record_query::project(e, row)
}
/// Reads one record, locking it for a write when asked.
pub async fn get(
    tx: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
    entity: &str,
    id: &str,
    write: bool,
) -> Result<Option<Value>> {
    Ok(veyndo_app_storage::records::get(tx, key(t, app, entity, id), write).await?)
}

/// Reads a bounded page of the records of an entity.
pub async fn rows(
    tx: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
    e: &Entity,
    limit: i64,
) -> Result<Vec<Value>> {
    let rows = veyndo_app_storage::records::rows(
        tx,
        veyndo_app_storage::Scope { tenant: t, app },
        &e.name,
        limit,
    )
    .await?;
    Ok(rows.iter().map(|row| project(e, row)).collect())
}

struct Compatibility;
impl veyndo_app_storage::models::Compatibility for Compatibility {
    type Error = Error;
    fn check(&self, old: &Entity, next: &Entity) -> Result<()> {
        core_extensions::compatible(old, next)
    }
}
/// Installs the entities of a package after checking them against the stored ones.
pub async fn install(tx: &mut sqlx::PgConnection, t: &str, m: &Manifest) -> Result<()> {
    veyndo_app_storage::models::install(
        tx,
        veyndo_app_storage::Scope {
            tenant: t,
            app: &m.id,
        },
        m,
        &Compatibility,
    )
    .await
    .map_err(|e| match e {
        veyndo_app_storage::models::Error::Database(e) => e.into(),
        veyndo_app_storage::models::Error::Invalid(s) => bad(s),
        veyndo_app_storage::models::Error::Conflict(s) => conflict(s),
        veyndo_app_storage::models::Error::Host(e) => e,
    })
}
/// Removes the preview records of an app.
pub async fn reset_preview(tx: &mut sqlx::PgConnection, t: &str, app: &str) -> Result<()> {
    Ok(
        veyndo_app_storage::records::reset_preview(
            tx,
            veyndo_app_storage::Scope { tenant: t, app },
        )
        .await?,
    )
}
