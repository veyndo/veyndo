//! Which categories a product is assigned to.
use crate::types::category::{Category, Edit};
use serde_json::json;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

pub async fn exists(tx: &mut PgConnection, tenant: &str, category: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant=$1 AND id=$2) AS "exists!""#,
        tenant,
        category
    )
    .fetch_one(tx)
    .await?)
}

/// Replaces the assignments of one product.
pub async fn replace(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    categories: &[String],
) -> Result<()> {
    sqlx::query!(
        r#"DELETE FROM product_categories WHERE tenant=$1 AND product_id=$2"#,
        tenant,
        product
    )
    .execute(&mut *tx)
    .await?;
    for category in categories {
        sqlx::query!(
            r#"INSERT INTO product_categories VALUES($1,$2,$3) ON CONFLICT DO NOTHING"#,
            tenant,
            product,
            category
        )
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

pub async fn of_product(a: &State, tenant: &str, product: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(r#"SELECT category_id FROM product_categories WHERE tenant=$1 AND product_id=$2 ORDER BY category_id"#, tenant, product)
        .fetch_all(&a.db)
        .await?)
}

/// The whole tree of a shop in display order; at most 2001 rows so the caller can refuse larger trees.
pub async fn tree(a: &State, tenant: &str) -> Result<Vec<Category>> {
    Ok(sqlx::query_as!(Category, r#"SELECT id,parent_id,position,revision,data FROM categories WHERE tenant=$1 ORDER BY position,id LIMIT 2001"#, tenant)
        .fetch_all(&a.db)
        .await?)
}

/// Serializes tree changes of one shop, so two moves cannot build a cycle together.
pub async fn lock_tree(tx: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query!(
        r#"SELECT pg_advisory_xact_lock(hashtextextended($1,726))"#,
        tenant
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Whether `parent` exists and is neither the category itself nor one of its descendants.
pub async fn parent_admissible(
    tx: &mut PgConnection,
    tenant: &str,
    parent: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"WITH RECURSIVE ancestors AS (SELECT id,parent_id FROM categories WHERE tenant=$1 AND id=$2 UNION ALL SELECT c.id,c.parent_id FROM categories c JOIN ancestors x ON c.id=x.parent_id WHERE c.tenant=$1) SELECT EXISTS(SELECT 1 FROM ancestors) AND NOT EXISTS(SELECT 1 FROM ancestors WHERE id=$3) AS "admissible!""#, tenant, parent, id)
        .fetch_one(tx)
        .await?)
}

pub async fn insert(tx: &mut PgConnection, tenant: &str, id: &str, edit: &Edit) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO categories(tenant,id,parent_id,position,data) VALUES($1,$2,$3,$4,$5)"#,
        tenant,
        id,
        edit.parent_id.as_deref(),
        edit.position,
        &edit.data
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Stores the edit at `revision`; `false` when the category changed or does not exist.
pub async fn update(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    edit: &Edit,
    revision: i64,
) -> Result<bool> {
    Ok(sqlx::query!(r#"UPDATE categories SET parent_id=$1,position=$2,data=$3,revision=revision+1 WHERE tenant=$4 AND id=$5 AND revision=$6"#, edit.parent_id.as_deref(), edit.position, &edit.data, tenant, id, revision)
        .execute(tx)
        .await?
        .rows_affected()
        == 1)
}

/// Gives a new shop its own root category and one category per legacy product group, with the products assigned.
pub async fn seed(tx: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE products SET product_number=id WHERE tenant=$1 AND product_number IS NULL"#,
        tenant
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(r#"INSERT INTO categories(tenant,id,data) VALUES($1,'catalog-root',$2) ON CONFLICT DO NOTHING"#, tenant, json!({"active":true,"visible":true,"type":"page","translations":{"en":{"name":"Catalog"},"de":{"name":"Katalog"},"fr":{"name":"Catalogue"},"es":{"name":"Catálogo"}}}))
        .execute(&mut *tx)
        .await?;
    sqlx::query!(r#"INSERT INTO categories(tenant,id,parent_id,data) SELECT DISTINCT tenant,'legacy-'||md5(category),'catalog-root',jsonb_build_object('active',true,'visible',true,'type','page','translations',jsonb_build_object('en',jsonb_build_object('name',initcap(category)),'de',jsonb_build_object('name',CASE category WHEN 'furniture' THEN 'Möbel' WHEN 'lighting' THEN 'Leuchten' WHEN 'objects' THEN 'Accessoires' ELSE initcap(category) END),'fr',jsonb_build_object('name',CASE category WHEN 'furniture' THEN 'Meubles' WHEN 'lighting' THEN 'Éclairage' WHEN 'objects' THEN 'Accessoires' ELSE initcap(category) END),'es',jsonb_build_object('name',CASE category WHEN 'furniture' THEN 'Muebles' WHEN 'lighting' THEN 'Iluminación' WHEN 'objects' THEN 'Accesorios' ELSE initcap(category) END))) FROM products WHERE tenant=$1 ON CONFLICT DO NOTHING"#, tenant)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(r#"INSERT INTO product_categories SELECT tenant,id,'legacy-'||md5(category) FROM products WHERE tenant=$1 ON CONFLICT DO NOTHING"#, tenant)
        .execute(tx)
        .await?;
    Ok(())
}

/// The active categories below and including `root`, in display order; at most 2001 rows.
pub async fn active_tree(a: &State, tenant: &str, root: &str) -> Result<Vec<Category>> {
    Ok(sqlx::query_as!(Category, r#"WITH RECURSIVE tree AS (SELECT * FROM categories WHERE tenant=$1 AND id=$2 AND data->>'active'='true' UNION ALL SELECT c.* FROM categories c JOIN tree p ON c.parent_id=p.id WHERE c.tenant=$1 AND c.data->>'active'='true') SELECT id AS "id!",parent_id,position AS "position!",revision AS "revision!",data AS "data!" FROM tree ORDER BY position,id LIMIT 2001"#, tenant, root)
        .fetch_all(&a.db)
        .await?)
}
