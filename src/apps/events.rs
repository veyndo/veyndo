//! App event delivery lives in the apps component; this module supplies its state to the worker and the outbox.
use super::service_limits;
use crate::{App, Result};
pub(crate) async fn deliver_once(a: &App) -> Result<bool> {
    veyndo_app_platform::service::event_delivery::deliver_once(&service_limits::platform(a)).await
}
pub(crate) use veyndo_app_platform::store::event_deliveries::project_events;
