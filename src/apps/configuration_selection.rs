//! Saved selections resolve in the apps component; this module adds the main language of the shop.
use super::Configuration;
use crate::{App, HashMap, Result, StoredCart, commerce};
pub(crate) use veyndo_app_platform::domain::configuration_selection::Selection;
use veyndo_app_platform::service::configuration_selection as resolver;
pub(crate) async fn resolve_selection(
    a: &App,
    t: &str,
    selection: &Selection,
) -> Result<Configuration> {
    let main_locale = commerce::config(a, t).await?.0.main_locale.clone();
    resolver::resolve_selection(a, t, selection, main_locale).await
}
pub(crate) async fn resolve_selections(
    a: &App,
    c: &StoredCart,
    selections: &[Selection],
) -> Result<HashMap<String, Configuration>> {
    let main_locale = commerce::config(a, &c.tenant).await?.0.main_locale.clone();
    resolver::resolve_selections(a, c, selections, &main_locale).await
}
