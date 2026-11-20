//! Bounded localized products for a model context; exact-ID hydration never falls back to a different catalogue.
use sqlx::Row;
use veyndo_catalog::store::products;
use veyndo_core::{Result, State};
use veyndo_pricing::domain::catalog::Product;

/// At most 24 parent products in the reader's language chain, names and descriptions bounded.
pub async fn localized_products(
    a: &State,
    t: &str,
    chain: &[String],
    ids: &[String],
    fallback: bool,
) -> Result<Vec<Product>> {
    // Empty retrieval still provides a stable bounded catalog for explicitly named IDs/fixtures.
    let rows=sqlx::query("SELECT p.* FROM products p WHERE p.tenant=$1 AND p.parent_id IS NULL AND (($3 AND cardinality($2::text[])=0) OR p.id=ANY($2)) ORDER BY coalesce(array_position($2,p.id),1000),p.id LIMIT 24").bind(t).bind(ids).bind(fallback).fetch_all(&a.db).await?;
    let mut ps = rows.iter().map(products::product).collect::<Vec<_>>();
    let ids = ps.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let rows=sqlx::query("SELECT product_id,language_id,name,description FROM product_translations WHERE tenant=$1 AND product_id=ANY($2) AND language_id=ANY($3)").bind(t).bind(ids).bind(chain).fetch_all(&a.db).await?;
    for p in &mut ps {
        for field in ["name", "description"] {
            for language in chain {
                if let Some(row) = rows.iter().find(|r| {
                    r.get::<String, _>("product_id") == p.id
                        && r.get::<String, _>("language_id") == *language
                }) && let Some(value) = row.get::<Option<String>, _>(field)
                {
                    if field == "name" {
                        p.name = value.chars().take(160).collect();
                    } else {
                        p.description = value.chars().take(1000).collect();
                    }
                    break;
                }
            }
        }
    }
    // Base-language fields are bounded as well.
    for p in &mut ps {
        p.name = p.name.chars().take(160).collect();
        p.description = p.description.chars().take(1000).collect();
    }
    Ok(ps)
}
