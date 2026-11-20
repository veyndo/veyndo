//! Controlled experiments use the existing storefront layout consumer and authoritative payment ledger; the intelligence component owns their store.
use crate::*;
use veyndo_intelligence::store::experiments as store;
pub(crate) use veyndo_intelligence::store::experiments::auto_once;
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "knowledge.read")?;
    if name == "knowledge.experiments" {
        return store::list(a, &t).await;
    }
    if name == "knowledge.experiment.suggest" {
        let (settings, _) = commerce::config(a, &t).await?;
        return store::suggest(
            a,
            &t,
            store::Basis {
                pricing_currency: &settings.currencies.pricing_currency,
                enabled: &settings.currencies.enabled,
                main_locale: &settings.main_locale,
                channel: marketing::channel_id(h),
            },
        )
        .await;
    }
    if name == "knowledge.experiment.report" {
        return store::report(a, &t, v["id"].as_str().ok_or(bad("Experiment required"))?).await;
    }
    auth::permit(h, "settings.write")?;
    if v["approve"] != true {
        return Err(bad("Explicit approve=true required"));
    }
    let actor = header(h, "x-rac-user").unwrap_or("integration");
    let currencies = if name == "knowledge.experiment.create" {
        commerce::config(a, &t).await?.0.currencies.enabled.clone()
    } else {
        vec![]
    };
    store::change(a, &t, name, v, actor, &currencies).await
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
