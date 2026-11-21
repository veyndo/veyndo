//! Controlled experiments use the existing storefront layout consumer and authoritative payment ledger; the intelligence component owns their store.
use crate::*;
use veyndo_intelligence::store::experiments as store;
pub(crate) use veyndo_intelligence::store::experiments::auto_once;
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::experiments::invoke(&a.core, h, name, v).await
}
/// Called only inside the native consent-locked experience transaction. Never overrides non-consenting layouts.
pub(crate) async fn assign(
    a: &App,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
) -> Result<Option<Value>> {
    let secret = env::var("PLATFORM_SECRET_KEY").unwrap_or_else(|_| a.token.to_string());
    store::assign(
        tx,
        &secret,
        store::Subject {
            tenant: &c.tenant,
            cart: &c.id,
            channel: &c.data.sales_channel,
            open: c.status == "open",
            currency: &c.data.currency,
        },
    )
    .await
}
