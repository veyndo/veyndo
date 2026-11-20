//! Native lifecycle/history hooks around the shared record mutation engine; package and typed field validation stay with the caller.
use crate::domain::{model_contract, persistence};
use crate::store::records;
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::{
    managed_hooks,
    mutations::{Mutation, WriteError},
};
use veyndo_core::{Error, Result, bad, conflict};

/// Writes one record through the shared mutation engine with lifecycle and history hooks.
pub async fn apply(
    tx: &mut sqlx::PgConnection,
    t: &str,
    m: &Manifest,
    e: &Entity,
    id: &str,
    value: &Value,
    replay: bool,
) -> Result<Value> {
    let names = e.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
    let mutation = Mutation {
        key: records::key(t, &m.id, &e.name, id),
        expected_revision: value["revision"].as_i64().unwrap_or(0),
        fields: &value["fields"],
        declared_fields: &names,
        replay,
    };
    veyndo_app_storage::mutations::write(
        tx,
        &mutation,
        &managed_hooks::Hooks {
            key: records::key(t, &m.id, &e.name, id),
            manifest: m,
            entity: e,
        },
    )
    .await
    .map_err(|error| match error {
        WriteError::Database(error) => error.into(),
        WriteError::Revision(current) => Error::revision(current, "App record revision changed"),
        WriteError::Conflict(message) => conflict(message),
        WriteError::Invalid(message) => bad(message),
        WriteError::Hook(managed_hooks::Error::Model(error)) => model_contract::error(error),
        WriteError::Hook(managed_hooks::Error::Storage(error)) => persistence::error(error),
    })
}
