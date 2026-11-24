//! Design reference capture is owned by the intelligence component; this entry lends the server's public egress.
use crate::*;
pub(crate) async fn capture(v: &Value) -> Result<(Value, Vec<String>)> {
    veyndo_intelligence::service::design_references::capture(&super::captions::Egress, v).await
}
