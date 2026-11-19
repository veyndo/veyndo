//! Storefront catalogue reads: the keyset-paged listing with its specialised queries, and bounded product lookups.
use crate::store::products::product;
use crate::types::catalog_page::CatalogCriteria;
use sqlx::Row;
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;

/// Up to `page size + 1` active main products visible to the channel and B2B unit of `criteria`.
/// `locale` and `main_locale` select the category names a saved fact query is matched in.
pub async fn page(
    a: &State,
    tenant: &str,
    chain: &[String],
    criteria: &CatalogCriteria,
    locale: &str,
    main_locale: &str,
) -> Result<Vec<Product>> {
    let limit = criteria.page_size()?;
    let category = criteria.category.as_deref().filter(|s| !s.is_empty());
    let after = criteria.after.as_deref().unwrap_or("");
    let search = criteria
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rows = if let Some(category_id) = &criteria.category_id {
        sqlx::query(include_str!("category_listing.sql"))
            .bind(tenant)
            .bind(category_id)
            .bind(after)
            .bind(&criteria.product_ids)
            .bind(search.map(|s| {
                format!(
                    "%{}%",
                    s.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                )
            }))
            .bind(chain)
            .bind((limit + 1) as i64)
            .bind(&criteria.channel_id)
            .bind(locale)
            .bind(main_locale)
            .bind(&criteria.b2b_unit)
            .fetch_all(&a.db)
            .await?
    } else if let Some(ids) = &criteria.product_ids {
        // Explicit channel catalogs contain at most 500 IDs. Filter before pagination;
        // the unscoped million-product path keeps its specialized indexed queries.
        let pattern = search.map(|s| {
            format!(
                "%{}%",
                s.to_lowercase()
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        });
        sqlx::query(include_str!("catalog_channel.sql"))
            .bind(tenant)
            .bind(ids)
            .bind(after)
            .bind(category)
            .bind(chain)
            .bind(pattern)
            .bind((limit + 1) as i64)
            .bind(&criteria.channel_id)
            .bind(&criteria.b2b_unit)
            .fetch_all(&a.db)
            .await?
    } else if let Some(search) = search {
        // Trigram candidates are indexed independently; check the effective
        // fallback-chain text before returning a match in the requested locale.
        let literal_pattern = |s: &str| {
            format!(
                "%{}%",
                s.to_lowercase()
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        };
        // A phrase can span a translated name and a fallback description.
        // Use one word for indexed candidates, then check the full effective
        // phrase. Very short searches retain exact semantics without a trigram.
        let candidate = search
            .split_whitespace()
            .max_by_key(|s| s.len())
            .filter(|s| s.chars().count() >= 3)
            .unwrap_or("");
        sqlx::query(include_str!("catalog_search.sql"))
            // Common and rare terms need different plans. A cached generic
            // plan regressed to a catalog scan after repeated mixed searches.
            .persistent(false)
            .bind(tenant)
            .bind(after)
            .bind(category)
            .bind(literal_pattern(search))
            .bind(chain)
            .bind((limit + 1) as i64)
            .bind(literal_pattern(candidate))
            .bind(&criteria.channel_id)
            .bind(&criteria.b2b_unit)
            .fetch_all(&a.db)
            .await?
    } else if let Some(category) = category {
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND parent_id IS NULL AND active AND id>$2 AND category=$3 AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=products.tenant AND v.product_id=products.id AND v.channel_id=$5 AND NOT v.visible)  AND ($6::text IS NULL OR EXISTS(SELECT 1 FROM b2b_catalog b WHERE b.tenant=products.tenant AND b.unit_id=$6 AND (b.product_id=products.id OR EXISTS(SELECT 1 FROM products child WHERE child.tenant=products.tenant AND child.parent_id=products.id AND child.id=b.product_id)))) ORDER BY id LIMIT $4")
            .bind(tenant).bind(after).bind(category).bind((limit+1) as i64).bind(&criteria.channel_id).bind(&criteria.b2b_unit).fetch_all(&a.db).await?
    } else {
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND parent_id IS NULL AND active AND id>$2 AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=products.tenant AND v.product_id=products.id AND v.channel_id=$4 AND NOT v.visible)  AND ($5::text IS NULL OR EXISTS(SELECT 1 FROM b2b_catalog b WHERE b.tenant=products.tenant AND b.unit_id=$5 AND (b.product_id=products.id OR EXISTS(SELECT 1 FROM products child WHERE child.tenant=products.tenant AND child.parent_id=products.id AND child.id=b.product_id)))) ORDER BY id LIMIT $3")
            .bind(tenant).bind(after).bind((limit+1) as i64).bind(&criteria.channel_id).bind(&criteria.b2b_unit).fetch_all(&a.db).await?
    };
    Ok(rows.iter().map(product).collect())
}

/// The given products in ID order.
pub async fn by_ids(a: &State, tenant: &str, ids: &[String]) -> Result<Vec<Product>> {
    Ok(
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id")
            .bind(tenant)
            .bind(ids)
            .fetch_all(&a.db)
            .await?
            .iter()
            .map(product)
            .collect(),
    )
}

pub async fn find(a: &State, tenant: &str, id: &str) -> Result<Option<Product>> {
    Ok(
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(tenant)
            .bind(id)
            .fetch_optional(&a.db)
            .await?
            .as_ref()
            .map(product),
    )
}

/// One page of the active variants of `family` visible to the channel and B2B unit, with the cursor to the next page.
pub async fn variants(
    a: &State,
    tenant: &str,
    family: &str,
    criteria: &CatalogCriteria,
    limit: usize,
) -> Result<(Vec<Product>, Option<String>)> {
    let allowed = criteria
        .product_ids
        .as_ref()
        .filter(|ids| !ids.iter().any(|id| id == family));
    let rows = sqlx::query(
        "SELECT * FROM products WHERE tenant=$1 AND parent_id=$2 AND active AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=products.tenant AND v.product_id=products.id AND v.channel_id=$6 AND NOT v.visible) AND id>$3 AND ($5::text[] IS NULL OR id=ANY($5)) AND ($7::text IS NULL OR EXISTS(SELECT 1 FROM b2b_catalog b WHERE b.tenant=products.tenant AND b.unit_id=$7 AND (b.product_id=products.id OR b.product_id=products.parent_id))) ORDER BY id LIMIT $4",
    )
    .bind(tenant)
    .bind(family)
    .bind(criteria.after.as_deref().unwrap_or(""))
    .bind((limit + 1) as i64)
    .bind(allowed)
    .bind(&criteria.channel_id)
    .bind(&criteria.b2b_unit)
    .fetch_all(&a.db)
    .await?;
    let next_cursor = (rows.len() > limit).then(|| rows[limit - 1].get::<String, _>("id"));
    Ok((rows.iter().take(limit).map(product).collect(), next_cursor))
}
