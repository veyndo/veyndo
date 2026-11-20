//! Action admission lives in the apps component; this module keeps the short path for the gateway.
use super::*;
pub(super) use veyndo_app_platform::domain::action_admission::Current;
pub(super) fn check(_a: &App, h: &RequestContext, action: &Action, value: &Value) -> Result<()> {
    veyndo_app_platform::domain::action_admission::check(h, action, value)
}
