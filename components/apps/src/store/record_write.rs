//! Native lifecycle/history hooks around the shared record mutation engine; package and typed field validation stay with the caller.
use crate::domain::{model_contract, persistence};
use crate::store::records;
use serde::Deserialize;
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::{
    managed_hooks,
    mutations::{Mutation, WriteError},
};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, bad, conflict};

/// The JSON fields `apply` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ApplyInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    fields: Value,
}

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
    let input: ApplyInput = view(value);
    let names = e.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
    let mutation = Mutation {
        key: records::key(t, &m.id, &e.name, id),
        expected_revision: input.revision.unwrap_or(0),
        fields: &input.fields,
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
