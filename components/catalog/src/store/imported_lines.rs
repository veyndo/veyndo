//! Products of an external order import: their parents, the locked lines, channel visibility and translated labels.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The distinct parents of the given products.
pub async fn parents(conn: &mut PgConnection, t: &str, ids: &[String]) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT parent_id AS "parent_id!" FROM products WHERE tenant=$1 AND id=ANY($2) AND parent_id IS NOT NULL"#,
        t,
        ids
    )
    .fetch_all(conn)
    .await?)
}

/// The product columns an imported order line reads.
pub struct LineProduct {
    pub id: String,
    pub name: String,
    pub stock: i32,
    pub parent_id: Option<String>,
    pub active: bool,
    pub product_number: Option<String>,
}

/// Locks the given products in ID order and returns the columns the order lines read.
pub async fn lock_products(
    conn: &mut PgConnection,
    t: &str,
    ids: &[String],
) -> Result<Vec<LineProduct>> {
    Ok(sqlx::query_as!(
        LineProduct,
        "SELECT id,name,stock,parent_id,active,product_number FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE",
        t,
        ids
    )
    .fetch_all(conn)
    .await?)
}

/// Whether one of the products is hidden in the sales channel.
pub async fn hidden_in_channel(
    conn: &mut PgConnection,
    t: &str,
    channel: &str,
    ids: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM product_channel_visibility WHERE tenant=$1 AND channel_id=$2 AND product_id=ANY($3) AND NOT visible) AS "hidden!""#,
        t,
        channel,
        ids
    )
    .fetch_one(conn)
    .await?)
}

/// A translated product name for an imported line.
pub struct Label {
    pub id: String,
    pub name: String,
}

/// The first non-blank translated name per product along the locale chain, the product's own before its parent's.
pub async fn labels(
    conn: &mut PgConnection,
    t: &str,
    ids: &[String],
    chain: &[String],
) -> Result<Vec<Label>> {
    Ok(
        sqlx::query_file_as!(Label, "src/store/imported_lines/labels.sql", t, ids, chain)
            .fetch_all(conn)
            .await?,
    )
}
