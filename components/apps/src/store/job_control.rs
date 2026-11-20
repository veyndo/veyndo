//! Native current-action/right/input admission for the shared job-control transaction; no duplicate transition or outbox engine.
use crate::domain::{action_admission, input_schema};
use crate::store::jobs;
use serde_json::Value;
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::job_control as shared;
use veyndo_core::{Error, RequestContext, StatusCode, bad, conflict};
use veyndo_identity::permit;

/// Readmits a retried job under the current action, rights and input schema.
pub struct Admission<'a> {
    pub context: &'a RequestContext,
    pub manifest: &'a Manifest,
}
impl<'db> shared::RetryAdmission<'db> for Admission<'_> {
    type Error = Error;
    fn admit<'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
        row: &'a sqlx::postgres::PgRow,
    ) -> shared::RetryFuture<'a, Self::Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            let action = self
                .manifest
                .actions
                .iter()
                .find(|a| a.name == row.get::<String, _>("action") && a.handler == "job")
                .ok_or(conflict("Job action is no longer installed"))?;
            permit(
                self.context,
                action.permission.as_deref().unwrap_or("apps.manage"),
            )?;
            action_admission::additional(self.context, action)?;
            input_schema::validate(&action.input_schema, &row.get::<Value, _>("input"))?;
            Ok(jobs::event_kind(self.manifest, &action.name))
        })
    }
}
/// The native error of a shared job control failure.
pub fn error(e: shared::ControlError<Error>) -> Error {
    match e {
        shared::ControlError::Database(e) => e.into(),
        shared::ControlError::Invalid(s) => bad(s),
        shared::ControlError::Conflict(s) => conflict(s),
        shared::ControlError::NotFound => Error(StatusCode::NOT_FOUND, "App job not found".into()),
        shared::ControlError::Hook(e) => e,
    }
}
