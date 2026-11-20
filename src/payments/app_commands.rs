//! App/Flow/MCP payment actions enqueue core jobs; provider-bound attempts prevent cross-app command authority.
use crate::{App, RequestContext, Result, Value, auth, merchant};

pub(crate) async fn app_command(
    a: &App,
    h: &RequestContext,
    provider: &str,
    v: &Value,
) -> Result<Value> {
    merchant(a, h)?;
    auth::permit(h, "payments.manage")?;
    veyndo_payments::service::commands::app_command(a, h, provider, v).await
}
pub(crate) use veyndo_payments::service::commands::command_action;
