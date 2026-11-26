//! Catalogue reads of shop memory and behaviour reports: owned product images for captions and product names along a language chain.
use sqlx::PgExecutor;
use veyndo_core::Result;

/// The bytes of an owned PNG, JPEG or WebP product image of at most 8 MiB.
pub async fn owned_image(
    e: impl PgExecutor<'_>,
    tenant: &str,
    id: &str,
) -> Result<Option<Vec<u8>>> {
    Ok(sqlx::query_scalar!("SELECT content FROM product_assets WHERE tenant=$1 AND id=$2 AND mime IN ('image/png','image/jpeg','image/webp') AND octet_length(content)<=8388608",
        tenant, id).fetch_optional(e).await?)
}

/// A product's first name along the language chain.
pub struct ChainName {
    pub product_id: String,
    pub name: String,
}

/// The named translations of the products in the chain's languages, in chain order.
pub async fn chain_names(
    e: impl PgExecutor<'_>,
    t: &str,
    ids: &[String],
    chain: &[String],
) -> Result<Vec<ChainName>> {
    Ok(sqlx::query_as!(ChainName, r#"SELECT product_id,name AS "name!" FROM product_translations WHERE tenant=$1 AND product_id=ANY($2) AND language_id=ANY($3) AND name IS NOT NULL ORDER BY array_position($3,language_id)"#,
        t, ids, chain).fetch_all(e).await?)
}

/// One product translation in a language of the chain.
pub struct ChainTranslation {
    pub product_id: String,
    pub language_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

/// The translations of the products in the chain's languages.
pub async fn chain_translations(
    e: impl PgExecutor<'_>,
    t: &str,
    ids: &[String],
    chain: &[String],
) -> Result<Vec<ChainTranslation>> {
    Ok(sqlx::query_as!(ChainTranslation, "SELECT product_id,language_id,name,description FROM product_translations WHERE tenant=$1 AND product_id=ANY($2) AND language_id=ANY($3)",
        t, ids, chain).fetch_all(e).await?)
}
