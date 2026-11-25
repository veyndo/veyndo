//! Bounded localized products for a model context; exact-ID hydration never falls back to a different catalogue.
use veyndo_catalog::store::products::ProductRow;
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
    let mut ps = sqlx::query_as!(ProductRow, "SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products p WHERE p.tenant=$1 AND p.parent_id IS NULL AND (($3 AND cardinality($2::text[])=0) OR p.id=ANY($2)) ORDER BY coalesce(array_position($2,p.id),1000),p.id LIMIT 24", t, ids, fallback)
        .fetch_all(&a.db)
        .await?
        .into_iter()
        .map(Product::from)
        .collect::<Vec<_>>();
    let ids = ps.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let rows = veyndo_catalog::store::intelligence_reads::chain_translations(&a.db, t, &ids, chain)
        .await?;
    for p in &mut ps {
        for field in ["name", "description"] {
            for language in chain {
                if let Some(row) = rows
                    .iter()
                    .find(|r| r.product_id == p.id && r.language_id == *language)
                    && let Some(value) = if field == "name" {
                        &row.name
                    } else {
                        &row.description
                    }
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
