//! Source content is validated by the intelligence component against the shop's enabled languages.
use super::*;
pub(super) async fn validate(a: &App, t: &str, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::sources::validate_content(&a.core, t, v).await
}
