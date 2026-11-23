//! Explicit merchant preferences reuse versioned private sources, read by the intelligence component; quoted guidance never expands native authority.
use super::*;
pub(crate) async fn current(a: &App, t: &str, locale: &str) -> Result<Value> {
    veyndo_intelligence::store::documents::guidance(a, t, locale).await
}
