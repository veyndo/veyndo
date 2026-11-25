//! Reads of translated product texts, always bounded to a given set of products.
use crate::types::translation::ProductText;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The texts of the given products in the languages of `chain`.
pub async fn for_products(
    a: &State,
    tenant: &str,
    chain: &[String],
    products: &[String],
) -> Result<Vec<ProductText>> {
    Ok(sqlx::query_as!(ProductText, r#"SELECT product_id,language_id,name,description FROM product_translations WHERE tenant=$1 AND language_id=ANY($2) AND product_id=ANY($3)"#, tenant, chain, products)
        .fetch_all(&a.db)
        .await?)
}

/// The same read inside a purchase, share-locked so the snapshot cannot change before commit.
pub async fn lock_for_products(
    tx: &mut PgConnection,
    tenant: &str,
    chain: &[String],
    products: &[String],
) -> Result<Vec<ProductText>> {
    Ok(sqlx::query_as!(ProductText, r#"SELECT product_id,language_id,name,description FROM product_translations WHERE tenant=$1 AND product_id=ANY($2) AND language_id=ANY($3) FOR SHARE"#, tenant, products, chain)
        .fetch_all(tx)
        .await?)
}

/// The texts of the given products in every language.
pub async fn all_for_products(
    a: &State,
    tenant: &str,
    products: &[String],
) -> Result<Vec<ProductText>> {
    Ok(sqlx::query_as!(ProductText, r#"SELECT product_id,language_id,name,description FROM product_translations WHERE tenant=$1 AND product_id=ANY($2)"#, tenant, products)
        .fetch_all(&a.db)
        .await?)
}
