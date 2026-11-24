//! Design reference capture is owned by the intelligence component; this entry lends the server's public egress.
use serde_json::Value;
use veyndo_core::Result;
pub(crate) async fn capture(v: &Value) -> Result<(Value, Vec<String>)> {
    veyndo_intelligence::service::design_references::capture(&super::captions::Egress, v).await
}
