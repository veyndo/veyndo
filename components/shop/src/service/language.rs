//! The language of a request from its shop, its locale header and an explicit language id.
use crate::domain::language_selection::select;
use crate::service::read_cache::{config, languages};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{RequestContext, Result, State};

/// The locale of the request and its translation chain.
pub async fn language_context(a: &State, h: &RequestContext) -> Result<(String, Vec<String>)> {
    let (settings, _) = config(a, &tenant(h)?).await?;
    let rows = languages(a).await?;
    select(
        &rows,
        &settings.main_locale,
        header(h, "x-commerce-locale"),
        header(h, "sw-language-id"),
    )
}
