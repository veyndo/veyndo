//! Shipping costs, proportional taxes and calendar delivery windows.
use super::*;
pub(crate) use veyndo_checkout::service::delivery::*;
pub(crate) async fn dates(a: &App, q: &mut Value) -> Result<()> {
    dates_conn(&mut *a.db.begin().await?, q).await
}
