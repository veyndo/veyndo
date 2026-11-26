//! The language chain of a stored cart, prepared before the checkout transaction holds a pool connection.
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::languages;
use veyndo_shop::types::cart::StoredCart;

/// The language ids and their locales the cart's products are translated in.
pub async fn language(a: &State, c: &StoredCart) -> Result<(Vec<String>, Vec<String>)> {
    let mut h = RequestContext::new();
    h.insert(
        "x-tenant",
        c.tenant.parse().map_err(|_| bad("Invalid tenant"))?,
    );
    if !c.data.locale.is_empty() {
        h.insert(
            "x-commerce-locale",
            c.data
                .locale
                .parse()
                .map_err(|_| bad("Invalid stored locale"))?,
        );
    }
    let (_, chain) = language_context(a, &h).await?;
    let registry = languages(a).await?;
    let locales = chain
        .iter()
        .filter_map(|id| {
            registry
                .iter()
                .find(|l| l.id == *id)
                .map(|l| l.locale.clone())
        })
        .collect();
    Ok((chain, locales))
}
