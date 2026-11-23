//! The run journal lives in the intelligence component; these entries lend it the server's agent port.
use crate::agent_host::Host;
use crate::*;
use veyndo_intelligence::service::run_journal as journal;
pub(crate) use veyndo_intelligence::service::run_journal::fence_commit;
pub(crate) async fn authorize(a: &App) -> Result<()> {
    journal::authorize(&Host(a)).await
}
pub(crate) async fn load(a: &App, key: &str) -> Result<Option<Value>> {
    journal::load(&Host(a), key).await
}
pub(crate) async fn persist(
    a: &App,
    t: &str,
    id: &str,
    evidence: &Value,
    h: &RequestContext,
) -> Result<()> {
    journal::persist(&Host(a), t, id, evidence, h).await
}
