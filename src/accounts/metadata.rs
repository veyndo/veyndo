//! Standard customer read fields; the projection and metrics are owned by the customers component.
use crate::*;
pub(crate) async fn decorate(a: &App, t: &str, email: &str, v: &mut Value) -> Result<()> {
    veyndo_customers::service::directory::decorate(&a.core, t, email, v).await
}
