//! Translated catalogue reads for the storefront, the cart and product pages; variants show their family's texts.
use crate::domain::family::inherit;
use crate::domain::hydration::hydrate;
use crate::ports::Catalog;
use crate::store::{catalog_reads, translations};
use crate::types::catalog_page::{CatalogCriteria, CatalogPage};
use std::collections::HashMap;
use veyndo_core::{Error, Result, StatusCode};
use veyndo_pricing::catalog::Product;

/// Translates products along the language chain; each field falls back on its own.
pub async fn localize(
    c: &Catalog,
    tenant: &str,
    chain: &[String],
    products: Vec<Product>,
) -> Result<Vec<Product>> {
    if products.is_empty() {
        return Ok(products);
    }
    let ids = products.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let texts = translations::for_products(&c.core, tenant, chain, &ids).await?;
    let locales = c.shop.locales(chain).await?;
    Ok(hydrate(products, &texts, chain, &locales))
}

/// One page of the storefront listing.
pub async fn page(
    c: &Catalog,
    tenant: &str,
    chain: &[String],
    criteria: &CatalogCriteria,
) -> Result<CatalogPage> {
    let limit = criteria.page_size()?;
    let (locale, main) = if criteria.category_id.is_some() {
        let main = c.shop.languages(tenant).await?.main;
        let locale = c
            .shop
            .locales(&chain[..chain.len().min(1)])
            .await?
            .into_iter()
            .next()
            .unwrap_or_else(|| main.clone());
        (locale, main)
    } else {
        (String::new(), String::new())
    };
    let mut rows = catalog_reads::page(&c.core, tenant, chain, criteria, &locale, &main).await?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let products = localize(c, tenant, chain, rows).await?;
    let next_cursor = has_more
        .then(|| products.last().map(|p| p.id.clone()))
        .flatten();
    Ok(CatalogPage {
        products,
        next_cursor,
        limit,
    })
}

/// The products of a cart with their parents, translated; a variant shows its parent's texts.
pub async fn for_cart(
    c: &Catalog,
    tenant: &str,
    chain: &[String],
    ids: &[String],
) -> Result<Vec<Product>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    // Two explicit indexed lookups avoid the OR/subquery plan that scanned a
    // million-row tenant even for a small cart. Root-only carts need one lookup.
    let mut selected = catalog_reads::by_ids(&c.core, tenant, ids).await?;
    let parent_ids = selected
        .iter()
        .filter_map(|p| p.parent_id.as_ref())
        .filter(|id| !ids.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    if !parent_ids.is_empty() {
        selected.extend(catalog_reads::by_ids(&c.core, tenant, &parent_ids).await?);
        selected.sort_by(|a, b| a.id.cmp(&b.id));
    }
    let roots = localize(
        c,
        tenant,
        chain,
        selected
            .iter()
            .filter(|p| p.parent_id.is_none())
            .cloned()
            .collect(),
    )
    .await?;
    let parents: HashMap<_, _> = roots.iter().map(|p| (p.id.as_str(), p)).collect();
    for p in &mut selected {
        let parent_id = p.parent_id.clone().unwrap_or_else(|| p.id.clone());
        if let Some(parent) = parents.get(parent_id.as_str()) {
            inherit(p, parent);
        }
    }
    localize(c, tenant, chain, selected).await
}

/// A product with its family: the root, one page of visible variants and always the requested product itself.
pub async fn family(
    c: &Catalog,
    tenant: &str,
    chain: &[String],
    id: &str,
    criteria: &CatalogCriteria,
) -> Result<(Vec<Product>, String, Option<String>)> {
    let limit = criteria.page_size()?;
    let selected = catalog_reads::find(&c.core, tenant, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Product not found".into()))?;
    let family = selected
        .parent_id
        .as_deref()
        .unwrap_or(&selected.id)
        .to_owned();
    let roots = catalog_reads::find(&c.core, tenant, &family)
        .await?
        .into_iter()
        .collect();
    let mut result = localize(c, tenant, chain, roots).await?;
    let (variants, next_cursor) =
        catalog_reads::variants(&c.core, tenant, &family, criteria, limit).await?;
    result.extend(variants);
    // A deep-linked SKU remains visible even when it is outside the requested page.
    if !result.iter().any(|p| p.id == id) {
        result.push(selected);
    }
    if let Some(root) = result.iter().find(|p| p.id == family).cloned() {
        for p in &mut result {
            if p.parent_id.as_deref() == Some(&family) {
                inherit(p, &root);
            }
        }
    }
    Ok((
        localize(c, tenant, chain, result).await?,
        family,
        next_cursor,
    ))
}
