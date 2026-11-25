//! Product identity reads that a private company catalogue is checked against: which requested products exist, their parents and whether a listed variant belongs to them, plus counts and product-number lookups. The B2B and marketing components evaluate their own catalogue lists against these rows. Columns checked at compile time.
use sqlx::PgConnection;
use veyndo_core::Result;

/// A requested product with its parent and whether one of its variants is among the listed ids.
pub struct ScopedProduct {
    pub id: String,
    pub parent_id: Option<String>,
    pub variant_listed: bool,
}

/// The products among `ids` that exist, each with its parent and whether a variant of it is in `listed`.
pub async fn scope(
    conn: &mut PgConnection,
    tenant: &str,
    ids: &[String],
    listed: &[String],
) -> Result<Vec<ScopedProduct>> {
    Ok(sqlx::query_as!(
        ScopedProduct,
        r#"SELECT p.id,p.parent_id,EXISTS(SELECT 1 FROM products child WHERE child.tenant=p.tenant AND child.parent_id=p.id AND child.id=ANY($3)) AS "variant_listed!" FROM products p WHERE p.tenant=$1 AND p.id=ANY($2)"#,
        tenant,
        ids,
        listed
    )
    .fetch_all(conn)
    .await?)
}

/// How many of `ids` are products of the shop.
pub async fn count(conn: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM products WHERE tenant=$1 AND id=ANY($2)"#,
        tenant,
        ids
    )
    .fetch_one(conn)
    .await?)
}

/// A product with its product number.
pub struct Sku {
    pub id: String,
    pub product_number: String,
}

/// The products whose numbers are among `skus`.
pub async fn by_sku(conn: &mut PgConnection, tenant: &str, skus: &[String]) -> Result<Vec<Sku>> {
    Ok(sqlx::query_as!(Sku, r#"SELECT id,product_number AS "product_number!" FROM products WHERE tenant=$1 AND product_number=ANY($2)"#, tenant, skus)
        .fetch_all(conn)
        .await?)
}
