//! App planning lives in the apps component; this module keeps the short path for the planner and proposals.
use super::{data, service_limits};
use crate::App;
use serde_json::Value;
pub(crate) use veyndo_app_platform::domain::planning::AppChange;
use veyndo_app_platform::store::planning;
pub(crate) use veyndo_app_platform::store::planning::planning_context;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn bind_change(
    a: &App,
    t: &str,
    c: &mut AppChange,
    h: &RequestContext,
) -> Result<()> {
    planning::bind_change(&service_limits::platform(a), t, c, h).await
}
pub(crate) async fn apply_change(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    c: &AppChange,
    h: &RequestContext,
) -> Result<()> {
    planning::apply_change(&data::BUNDLED, tx, t, c, h).await
}
pub(crate) async fn planning_contracts(
    a: &App,
    t: &str,
    h: &RequestContext,
    ids: &[String],
) -> Result<Value> {
    planning::contracts(&service_limits::platform(a), t, h, ids).await
}
