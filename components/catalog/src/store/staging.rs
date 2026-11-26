//! Catalogue units of a sandbox shop: copying products, translations, assets, categories and channel visibility, locked snapshot reads and release writes, all inside the caller's transaction.
use serde_json::Value;
use veyndo_core::Result;

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

/// Copies the live catalogue (products, translations, assets, categories) into the sandbox.
pub async fn copy_catalog(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!("INSERT INTO products SELECT (jsonb_populate_record(NULL::products,to_jsonb(p)||jsonb_build_object('tenant',$2::text))).* FROM products p WHERE tenant=$1", t, id).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO product_translations SELECT $2,product_id,language_id,name,description FROM product_translations WHERE tenant=$1", t, id).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO product_assets SELECT (jsonb_populate_record(NULL::product_assets,to_jsonb(a)||jsonb_build_object('tenant',$2::text))).* FROM product_assets a WHERE tenant=$1", t, id).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO categories SELECT $2,id,parent_id,position,data,revision FROM categories WHERE tenant=$1", t, id).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO product_categories SELECT $2,product_id,category_id FROM product_categories WHERE tenant=$1", t, id).execute(&mut *c).await?;
    Ok(())
}

/// Every product as a document without tenant, stock and revision clocks, locked.
pub async fn products(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(p)-'tenant'-'stock'-'revision'-'content_revision' AS "data!" FROM products p WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut *c).await?)
}

/// One product as a document without tenant, stock and revision clocks, locked.
pub async fn product(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(p)-'tenant'-'stock'-'revision'-'content_revision' AS "value!" FROM products p WHERE tenant=$1 AND id=$2 FOR UPDATE"#, t, id).fetch_one(&mut *c).await?)
}

/// A product's translations as `(language, name, description)`, locked.
pub async fn translations(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    Ok(sqlx::query!(r#"SELECT language_id,name AS "name?",description AS "description?" FROM product_translations WHERE tenant=$1 AND product_id=$2 ORDER BY language_id FOR UPDATE"#, t, id).fetch_all(&mut *c).await?.into_iter().map(|r| (r.language_id, r.name, r.description)).collect())
}

/// A product's category IDs.
pub async fn category_ids(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT category_id FROM product_categories WHERE tenant=$1 AND product_id=$2 ORDER BY category_id", t, id).fetch_all(&mut *c).await?)
}

/// A product's channel visibility as `(channel, visible)`.
pub async fn channel_visibility(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Vec<(String, bool)>> {
    Ok(sqlx::query!("SELECT channel_id,visible FROM product_channel_visibility WHERE tenant=$1 AND product_id=$2 ORDER BY channel_id", t, id).fetch_all(&mut *c).await?.into_iter().map(|r| (r.channel_id, r.visible)).collect())
}

/// Inserts a product with zero inventory unless it already exists.
pub async fn insert_product(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    p: &NewProduct<'_>,
) -> Result<()> {
    sqlx::query!("INSERT INTO products(tenant,id,name,category,description,price,tax_rate,stock,active,product_number) VALUES($1,$2,$3,$4,$5,$6,$7,0,$8,$9) ON CONFLICT(tenant,id) DO NOTHING", t, id, p.name, p.category, p.description, p.price, p.tax_rate, p.active, p.product_number).execute(&mut *c).await?;
    Ok(())
}

/// Overwrites a product's content columns from a document through an explicit allowlist; returns the rows changed.
pub async fn update_product_content(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    value: &Value,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE products SET (name,category,description,price,tax_rate,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,active,product_number)=(SELECT x.name,x.category,x.description,x.price,x.tax_rate,x.list_price,x.regulation_price,x.reference_price,x.advanced_prices,x.min_purchase,x.purchase_steps,x.max_purchase,x.parent_id,x.options,x.media,x.properties,x.delivery_days,x.extra,x.active,x.product_number FROM jsonb_populate_record(NULL::products,$1) x),revision=revision+1 WHERE tenant=$2 AND id=$3", value, t, id).execute(&mut *c).await?.rows_affected())
}

/// Removes a product's channel visibility rows.
pub async fn clear_channel_visibility(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM product_channel_visibility WHERE tenant=$1 AND product_id=$2",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Adds one channel visibility row of a product.
pub async fn insert_channel_visibility(
    c: &mut sqlx::PgConnection,
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
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Inserts or replaces one product translation.
pub async fn upsert_translation(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    language: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    sqlx::query!("INSERT INTO product_translations(tenant,product_id,language_id,name,description) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,product_id,language_id) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description", t, id, language, name, description).execute(&mut *c).await?;
    Ok(())
}

/// Whether a product exists in the shop.
pub async fn product_exists(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&mut *c)
    .await?)
}

/// Copies product channel visibility into the sandbox.
pub async fn copy_channel_visibility(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!("INSERT INTO product_channel_visibility SELECT $2,product_id,channel_id,visible FROM product_channel_visibility WHERE tenant=$1", t, id).execute(&mut *c).await?;
    Ok(())
}

/// Every asset as a document without tenant, content and creation time, locked.
pub async fn assets(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(a)-'tenant'-'content'-'created_at' AS "data!" FROM product_assets a WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut *c).await?)
}

/// The live product a staged asset belongs to, share-locked; `None` while the product is not live.
pub async fn asset_product(
    c: &mut sqlx::PgConnection,
    live: &str,
    stage: &str,
    id: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!("SELECT p.id FROM product_assets a JOIN products p ON p.tenant=$1 AND p.id=a.product_id WHERE a.tenant=$2 AND a.id=$3 FOR SHARE OF p", live, stage, id).fetch_optional(&mut *c).await?)
}

/// Copies one staged asset into the live shop or updates its visibility and title.
pub async fn publish_asset(
    c: &mut sqlx::PgConnection,
    live: &str,
    stage: &str,
    id: &str,
) -> Result<()> {
    sqlx::query!("INSERT INTO product_assets(tenant,id,product_id,title,filename,mime,kind,public,content,digest) SELECT $1,id,product_id,title,filename,mime,kind,public,content,digest FROM product_assets WHERE tenant=$2 AND id=$3 ON CONFLICT(tenant,id) DO UPDATE SET public=EXCLUDED.public,title=EXCLUDED.title", live, stage, id).execute(&mut *c).await?;
    Ok(())
}

/// A product's media list.
pub async fn product_media(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT media FROM products WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_one(&mut *c)
    .await?)
}

/// Whether a public attachment asset exists, share-locked.
pub async fn public_attachment(
    c: &mut sqlx::PgConnection,
    t: &str,
    asset: &str,
) -> Result<Option<bool>> {
    Ok(sqlx::query_scalar!(r#"SELECT true AS "found!" FROM product_assets WHERE tenant=$1 AND id=$2 AND public AND kind='attachment' FOR SHARE"#, t, asset)
    .fetch_optional(&mut *c)
    .await?)
}

/// Every category as a document without tenant and revision, locked.
pub async fn categories(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(c)-'tenant'-'revision' AS "data!" FROM categories c WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut *c).await?)
}

/// Whether a category exists.
pub async fn category_exists(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&mut *c)
    .await?)
}

/// Whether `id` is among the ancestors of `parent` (including `parent` itself).
pub async fn category_cycle(
    c: &mut sqlx::PgConnection,
    t: &str,
    parent: &str,
    id: Option<&str>,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"WITH RECURSIVE parents AS (SELECT id,parent_id FROM categories WHERE tenant=$1 AND id=$2 UNION ALL SELECT c.id,c.parent_id FROM categories c JOIN parents p ON c.id=p.parent_id WHERE c.tenant=$1) SELECT EXISTS(SELECT 1 FROM parents WHERE id=$3) AS "exists!""#, t, parent, id).fetch_one(&mut *c).await?)
}

/// Inserts or replaces a category at the next revision.
pub async fn upsert_category(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: Option<&str>,
    parent: Option<&str>,
    position: i32,
    data: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO categories(tenant,id,parent_id,position,data) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET parent_id=EXCLUDED.parent_id,position=EXCLUDED.position,data=EXCLUDED.data,revision=categories.revision+1", t, id, parent, position, data).execute(&mut *c).await?;
    Ok(())
}
