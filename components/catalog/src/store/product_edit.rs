//! Writes of one product edit inside its transaction: allocation lock, creation, clocks, identity, content and translations.
use crate::store::products::product;
use crate::types::product_edit::CatalogFields;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;

/// Serializes product number allocation for this shop, not globally.
pub async fn lock_numbers(tx: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,727))")
        .bind(tenant)
        .execute(tx)
        .await?;
    Ok(())
}

pub async fn is_main_product(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2 AND parent_id IS NULL)",
    )
    .bind(tenant)
    .bind(id)
    .fetch_one(tx)
    .await?)
}

/// Creates the empty product row; `false` when the ID is taken.
pub async fn insert(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    name: &str,
    description: &str,
    parent: Option<&str>,
) -> Result<bool> {
    Ok(sqlx::query("INSERT INTO products(tenant,id,name,description,category,price,tax_rate,stock,revision,content_revision,parent_id) VALUES($1,$2,$3,$4,'objects',0,19,0,0,0,$5) ON CONFLICT(tenant,id) DO NOTHING")
        .bind(tenant)
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(parent)
        .execute(tx)
        .await?
        .rows_affected()
        == 1)
}

/// The clocks and the variant link of a product, locked for this edit.
pub struct Clocks {
    pub revision: i64,
    pub content_revision: i64,
    pub parent_id: Option<String>,
}

pub async fn lock_clocks(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<Clocks>> {
    Ok(sqlx::query(
        "SELECT revision,content_revision,parent_id FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE",
    )
    .bind(tenant)
    .bind(id)
    .fetch_optional(tx)
    .await?
    .map(|row| Clocks {
        revision: row.get("revision"),
        content_revision: row.get("content_revision"),
        parent_id: row.get("parent_id"),
    }))
}

pub async fn number_taken(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    number: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND product_number=$2 AND id<>$3)",
    )
    .bind(tenant)
    .bind(number)
    .bind(id)
    .fetch_one(tx)
    .await?)
}

/// Whether another variant of the parent already has this option combination.
pub async fn options_taken(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    parent: &str,
    options: &Value,
) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND parent_id=$2 AND id<>$3 AND options=$4)")
        .bind(tenant)
        .bind(parent)
        .bind(id)
        .bind(options)
        .fetch_one(tx)
        .await?)
}

pub async fn update_identity(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    fields: &CatalogFields,
) -> Result<()> {
    sqlx::query(
        "UPDATE products SET active=$1,product_number=$2,options=$3 WHERE tenant=$4 AND id=$5",
    )
    .bind(fields.active)
    .bind(&fields.product_number)
    .bind(json!(fields.options))
    .bind(tenant)
    .bind(id)
    .execute(tx)
    .await?;
    Ok(())
}

/// Stores metadata and the main texts and advances the aggregate clock; `false` when the clock moved.
pub async fn update_content(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    extra: Value,
    name: &str,
    description: &str,
    revision: i64,
) -> Result<bool> {
    Ok(sqlx::query("UPDATE products SET extra=$1,name=$2,description=$3,revision=revision+1 WHERE tenant=$4 AND id=$5 AND revision=$6")
        .bind(extra)
        .bind(name)
        .bind(description)
        .bind(tenant)
        .bind(id)
        .bind(revision)
        .execute(tx)
        .await?
        .rows_affected()
        == 1)
}

/// Stores the texts of one language; `None` fields inherit the main language.
pub async fn upsert_translation(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    locale: &str,
    name: Option<String>,
    description: Option<String>,
) -> Result<()> {
    sqlx::query("INSERT INTO product_translations(tenant,product_id,language_id,name,description) SELECT $1,$2,id,$3,$4 FROM languages WHERE locale=$5 ON CONFLICT(tenant,product_id,language_id) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description")
        .bind(tenant)
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(locale)
        .execute(tx)
        .await?;
    Ok(())
}

/// The product as stored after the edit, with its content clock.
pub async fn saved(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<(Product, i64)> {
    let row = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_one(tx)
        .await?;
    Ok((product(&row), row.get("content_revision")))
}
