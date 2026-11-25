//! SQL of the studio's product reads and writes: shared product rows for proposals and the editor, reviewed price/stock changes and existence checks.
use super::staging_documents::Tx;
use crate::domain::proposal::Change;
use serde_json::Value;
use sqlx::postgres::PgRow;
use veyndo_catalog::store::products::product;
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;

/// Up to 100 of the requested products.
pub async fn requested(a: &State, t: &str, ids: &[String]) -> Result<Vec<Product>> {
    // dynamic SQL: the whole row is decoded by the catalog's shared `product(&PgRow)` mapper.
    Ok(
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id LIMIT 100")
            .bind(t)
            .bind(ids)
            .fetch_all(&a.db)
            .await?
            .iter()
            .map(product)
            .collect(),
    )
}

/// The given products, locked in ID order.
pub async fn lock(tx: &mut Tx<'_>, t: &str, ids: Vec<String>) -> Result<Vec<Product>> {
    // dynamic SQL: the whole row is decoded by the catalog's shared `product(&PgRow)` mapper.
    Ok(
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE")
            .bind(t)
            .bind(ids)
            .fetch_all(&mut **tx)
            .await?
            .iter()
            .map(product)
            .collect(),
    )
}

/// One product inside a transaction.
pub async fn get(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Product> {
    // dynamic SQL: the whole row is decoded by the catalog's shared `product(&PgRow)` mapper.
    let row = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(product(&row))
}

/// The full product row the editor projects through the catalog's row readers.
pub async fn editor_row(a: &State, t: &str, id: &str) -> Result<Option<PgRow>> {
    // dynamic SQL: the whole row is decoded by the catalog's shared `editable(&PgRow)` reader.
    Ok(
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?,
    )
}

/// A product's translation in each given locale as `(locale, name, description)`; missing ones are `None`.
pub async fn translations(
    a: &State,
    t: &str,
    id: &str,
    locales: &[String],
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    Ok(sqlx::query!(r#"SELECT l.locale,p.name AS "name?",p.description AS "description?" FROM languages l LEFT JOIN product_translations p ON p.language_id=l.id AND p.tenant=$1 AND p.product_id=$2 WHERE l.locale=ANY($3)"#, t, id, locales).fetch_all(&a.db).await?.into_iter().map(|r| (r.locale, r.name, r.description)).collect())
}

/// Applies a reviewed price/stock change at its expected revision; returns the rows changed.
pub async fn apply_change(tx: &mut Tx<'_>, t: &str, c: &Change) -> Result<u64> {
    Ok(sqlx::query!("UPDATE products SET price=COALESCE($1,price),stock=COALESCE($2,stock),revision=revision+1 WHERE tenant=$3 AND id=$4 AND revision=$5", c.price, c.stock, t, &c.product_id, c.expected_revision).execute(&mut **tx).await?.rows_affected())
}

/// Whether a product exists.
pub async fn exists(a: &State, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&a.db)
    .await?)
}

/// Whether a category exists.
pub async fn category_exists(a: &State, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&a.db)
    .await?)
}

/// Replaces the storefront experience at its expected revision; returns the rows changed.
pub async fn replace_experience(
    tx: &mut Tx<'_>,
    t: &str,
    data: Value,
    expected: Option<i64>,
) -> Result<u64> {
    Ok(sqlx::query!(
        "UPDATE experiences SET data=$1,revision=revision+1 WHERE tenant=$2 AND revision=$3",
        data,
        t,
        expected
    )
    .execute(&mut **tx)
    .await?
    .rows_affected())
}

/// The settings document and its revision, locked for update.
pub async fn lock_settings(tx: &mut Tx<'_>, t: &str) -> Result<(Value, i64)> {
    let row = sqlx::query!(
        "SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok((row.data, row.revision))
}

/// The settings document and its revision, share-locked.
pub async fn share_settings(tx: &mut Tx<'_>, t: &str) -> Result<(Value, i64)> {
    let row = sqlx::query!(
        "SELECT data, revision FROM commerce_settings WHERE tenant=$1 FOR SHARE",
        t
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok((row.data, row.revision))
}
