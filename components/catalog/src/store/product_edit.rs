//! Writes of one product edit inside its transaction: allocation lock, creation, clocks, identity, content and translations.
use crate::store::products::ProductRow;
use crate::types::product_edit::CatalogFields;
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;

/// Serializes product number allocation for this shop, not globally.
pub async fn lock_numbers(tx: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query!(
        r#"SELECT pg_advisory_xact_lock(hashtextextended($1,727))"#,
        tenant
    )
    .execute(tx)
    .await?;
    Ok(())
}

pub async fn is_main_product(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2 AND parent_id IS NULL) AS "exists!""#, tenant, id)
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
    Ok(sqlx::query!(r#"INSERT INTO products(tenant,id,name,description,category,price,tax_rate,stock,revision,content_revision,parent_id) VALUES($1,$2,$3,$4,'objects',0,19,0,0,0,$5) ON CONFLICT(tenant,id) DO NOTHING"#, tenant, id, name, description, parent)
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
    Ok(sqlx::query_as!(Clocks, r#"SELECT revision,content_revision,parent_id FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE"#, tenant, id)
    .fetch_optional(tx)
    .await?)
}

pub async fn number_taken(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    number: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND product_number=$2 AND id<>$3) AS "exists!""#, tenant, number, id)
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
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND parent_id=$2 AND id<>$3 AND options=$4) AS "exists!""#, tenant, parent, id, options)
        .fetch_one(tx)
        .await?)
}

pub async fn update_identity(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    fields: &CatalogFields,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE products SET active=$1,product_number=$2,options=$3 WHERE tenant=$4 AND id=$5"#,
        fields.active,
        &fields.product_number,
        json!(fields.options),
        tenant,
        id
    )
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
    Ok(sqlx::query!(r#"UPDATE products SET extra=$1,name=$2,description=$3,revision=revision+1 WHERE tenant=$4 AND id=$5 AND revision=$6"#, extra, name, description, tenant, id, revision)
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
    // An unregistered locale stores nothing.
    let Some(language) = crate::ports::registries()?
        .language_id(&mut *tx, locale)
        .await?
    else {
        return Ok(());
    };
    sqlx::query!(r#"INSERT INTO product_translations(tenant,product_id,language_id,name,description) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,product_id,language_id) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description"#, tenant, id, language, name, description)
        .execute(tx)
        .await?;
    Ok(())
}

/// The product as stored after the edit, with its content clock.
pub async fn saved(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<(Product, i64)> {
    let row = sqlx::query!(r#"SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number,content_revision FROM products WHERE tenant=$1 AND id=$2"#, tenant, id)
        .fetch_one(tx)
        .await?;
    let product = ProductRow {
        id: row.id,
        name: row.name,
        category: row.category,
        description: row.description,
        price: row.price,
        tax_rate: row.tax_rate,
        stock: row.stock,
        revision: row.revision,
        list_price: row.list_price,
        regulation_price: row.regulation_price,
        reference_price: row.reference_price,
        advanced_prices: row.advanced_prices,
        min_purchase: row.min_purchase,
        purchase_steps: row.purchase_steps,
        max_purchase: row.max_purchase,
        parent_id: row.parent_id,
        options: row.options,
        media: row.media,
        properties: row.properties,
        delivery_days: row.delivery_days,
        extra: row.extra,
        product_number: row.product_number,
    };
    Ok((product.into(), row.content_revision))
}
