//! App Studio builds live in the apps component; this module supplies the shop main language and the payment provider actions.
use super::*;
use veyndo_app_platform::service::app_builds::{self, Origin};
pub(super) async fn save(
    a: &App,
    t: &str,
    v: &Value,
    provider: &str,
    model: Option<&str>,
    evidence: Option<&Value>,
) -> Result<Value> {
    let (settings, _) = commerce::config(a, t).await?;
    app_builds::save(
        &a.core,
        t,
        v,
        &Origin {
            provider,
            model,
            evidence,
            main: &settings.main_locale,
            payment: &routes::payment_actions(),
        },
    )
    .await
}
