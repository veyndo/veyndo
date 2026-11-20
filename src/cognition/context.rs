//! Bounded localized catalog retrieval before inference; full catalog size never expands the prompt.
use super::*;
pub(crate) use veyndo_intelligence::domain::prompt_context::{
    SOURCE_POLICY, catalog_snapshot, snapshot,
};
pub(crate) use veyndo_intelligence::store::context_products::localized_products;
pub(crate) async fn context_products(
    a: &App,
    t: &str,
    chain: &[String],
    query: &str,
) -> Result<Vec<Product>> {
    Ok(context_retrieval(a, t, chain, query).await?.0)
}
/// Share one retrieval between native localized products and the advisor's relevant graph.
pub(crate) async fn context_retrieval(
    a: &App,
    t: &str,
    chain: &[String],
    query: &str,
) -> Result<(Vec<Product>, Value)> {
    let search = crate::agent::retrieve(a, t, query).await?;
    let hits = search["hits"].as_array().cloned().unwrap_or_default();
    let ids = hits
        .iter()
        .filter_map(|v| v["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    Ok((localized_products(a, t, chain, &ids, true).await?, search))
}
