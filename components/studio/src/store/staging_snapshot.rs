//! SQL of staging snapshots and product publication: locked reads of every publishable unit and the native product content writes.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::Result;

/// Locks the settings row first, in the same order as native organization writes.
pub async fn lock_settings(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(())
}

/// Every product as a document without tenant, stock and revision clocks, locked.
pub async fn products(tx: &mut Tx<'_>, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(p)-'tenant'-'stock'-'revision'-'content_revision' AS "data!" FROM products p WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut **tx).await?)
}

/// One product as a document without tenant, stock and revision clocks, locked.
pub async fn product(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(p)-'tenant'-'stock'-'revision'-'content_revision' AS "value!" FROM products p WHERE tenant=$1 AND id=$2 FOR UPDATE"#, t, id).fetch_one(&mut **tx).await?)
}

/// The settings document, locked.
pub async fn settings(tx: &mut Tx<'_>, t: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut **tx)
    .await?)
}

/// The storefront experience document, locked.
pub async fn experience(tx: &mut Tx<'_>, t: &str) -> Result<Option<Value>> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM experiences WHERE tenant=$1 FOR UPDATE", t)
            .fetch_optional(&mut **tx)
            .await?,
    )
}

/// One installed app package.
pub struct Package {
    pub id: String,
    pub manifest: Value,
    pub active: bool,
}

/// Every installed app package, locked.
pub async fn packages(tx: &mut Tx<'_>, t: &str) -> Result<Vec<Package>> {
    Ok(sqlx::query_as!(
        Package,
        "SELECT id,manifest,active FROM app_packages WHERE tenant=$1 ORDER BY id FOR UPDATE",
        t
    )
    .fetch_all(&mut **tx)
    .await?)
}

/// The release units of one kind (`rule`, `promotion`, `flow`, `channel`) by ID, locked.
pub async fn units(tx: &mut Tx<'_>, t: &str, kind: &str) -> Result<Vec<(String, Value)>> {
    Ok(match kind {
        "rule" => sqlx::query!(r#"SELECT id,jsonb_build_object('name',name,'condition',condition,'active',active) AS "data!" FROM commerce_rules WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut **tx).await?.into_iter().map(|r| (r.id, r.data)).collect(),
        "promotion" => sqlx::query!("SELECT id,data FROM commerce_promotions WHERE tenant=$1 ORDER BY id FOR UPDATE", t).fetch_all(&mut **tx).await?.into_iter().map(|r| (r.id, r.data)).collect(),
        "flow" => sqlx::query!("SELECT id,data FROM commerce_flows WHERE tenant=$1 ORDER BY id FOR UPDATE", t).fetch_all(&mut **tx).await?.into_iter().map(|r| (r.id, r.data)).collect(),
        _ => sqlx::query!("SELECT id,data FROM sales_channels WHERE tenant=$1 ORDER BY id FOR UPDATE", t).fetch_all(&mut **tx).await?.into_iter().map(|r| (r.id, r.data)).collect(),
    })
}

/// The order workflow document, locked.
pub async fn order_workflow(tx: &mut Tx<'_>, t: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM order_state_machines WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut **tx)
    .await?)
}

/// A product's translations as `(language, name, description)`, locked.
pub async fn translations(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    Ok(sqlx::query!(r#"SELECT language_id,name AS "name?",description AS "description?" FROM product_translations WHERE tenant=$1 AND product_id=$2 ORDER BY language_id FOR UPDATE"#, t, id).fetch_all(&mut **tx).await?.into_iter().map(|r| (r.language_id, r.name, r.description)).collect())
}

/// A product's category IDs.
pub async fn category_ids(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT category_id FROM product_categories WHERE tenant=$1 AND product_id=$2 ORDER BY category_id", t, id).fetch_all(&mut **tx).await?)
}

/// A product's channel visibility as `(channel, visible)`.
pub async fn channel_visibility(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Vec<(String, bool)>> {
    Ok(sqlx::query!("SELECT channel_id,visible FROM product_channel_visibility WHERE tenant=$1 AND product_id=$2 ORDER BY channel_id", t, id).fetch_all(&mut **tx).await?.into_iter().map(|r| (r.channel_id, r.visible)).collect())
}

/// The columns a newly staged product enters live with; stock starts at zero.
pub struct NewProduct<'a> {
    pub name: Option<&'a str>,
    pub category: Option<&'a str>,
    pub description: Option<&'a str>,
    pub price: Option<f64>,
    pub tax_rate: Option<f64>,
    pub active: bool,
    pub product_number: Option<&'a str>,
}

/// Inserts a product with zero inventory unless it already exists.
pub async fn insert_product(tx: &mut Tx<'_>, t: &str, id: &str, p: &NewProduct<'_>) -> Result<()> {
    sqlx::query!("INSERT INTO products(tenant,id,name,category,description,price,tax_rate,stock,active,product_number) VALUES($1,$2,$3,$4,$5,$6,$7,0,$8,$9) ON CONFLICT(tenant,id) DO NOTHING", t, id, p.name, p.category, p.description, p.price, p.tax_rate, p.active, p.product_number).execute(&mut **tx).await?;
    Ok(())
}

/// Overwrites a product's content columns from a document through an explicit allowlist; returns the rows changed.
pub async fn update_product_content(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    value: &Value,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE products SET (name,category,description,price,tax_rate,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,active,product_number)=(SELECT x.name,x.category,x.description,x.price,x.tax_rate,x.list_price,x.regulation_price,x.reference_price,x.advanced_prices,x.min_purchase,x.purchase_steps,x.max_purchase,x.parent_id,x.options,x.media,x.properties,x.delivery_days,x.extra,x.active,x.product_number FROM jsonb_populate_record(NULL::products,$1) x),revision=revision+1 WHERE tenant=$2 AND id=$3", value, t, id).execute(&mut **tx).await?.rows_affected())
}

/// Removes a product's channel visibility rows.
pub async fn clear_channel_visibility(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM product_channel_visibility WHERE tenant=$1 AND product_id=$2",
        t,
        id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Adds one channel visibility row of a product.
pub async fn insert_channel_visibility(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    channel: Option<&str>,
    visible: Option<bool>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO product_channel_visibility VALUES($1,$2,$3,$4)",
        t,
        id,
        channel,
        visible
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Inserts or replaces one product translation.
pub async fn upsert_translation(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    language: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    sqlx::query!("INSERT INTO product_translations(tenant,product_id,language_id,name,description) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,product_id,language_id) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description", t, id, language, name, description).execute(&mut **tx).await?;
    Ok(())
}

/// Whether a product exists in the shop.
pub async fn product_exists(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&mut **tx)
    .await?)
}
