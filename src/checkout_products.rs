//! The locked SKU snapshot is owned by the checkout component; the language chain is prepared here before the transaction.
use crate::*;
pub(crate) use veyndo_checkout::store::product_snapshot::snapshot;

/// Prepare language metadata before the checkout holds a pool connection.
pub(crate) async fn language(a: &App, c: &StoredCart) -> Result<(Vec<String>, Vec<String>)> {
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
    let languages = performance::languages(a).await?;
    let locales = chain
        .iter()
        .filter_map(|id| {
            languages
                .iter()
                .find(|l| l.id == *id)
                .map(|l| l.locale.clone())
        })
        .collect();
    Ok((chain, locales))
}
