//! Catalogue reads of the app platform: whether a product or asset an app record or surface names belongs to the shop, and a product's evidence text.
use serde_json::Value;
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// Whether the shop has the product.
pub async fn product_exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "owned!""#,
        t,
        id
    )
    .fetch_one(db)
    .await?)
}

/// The products among `ids` that the shop has.
pub async fn existing_products<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    ids: &[String],
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM products WHERE tenant=$1 AND id=ANY($2)",
        t,
        ids
    )
    .fetch_all(db)
    .await?)
}

/// Whether an asset of the shop fits an app field: an image for an image field, a public attachment for a public entity.
pub async fn asset_fits<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
    kind: &str,
    public: bool,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM product_assets WHERE tenant=$1 AND id=$2 AND ($3::text<> 'image' OR mime LIKE 'image/%') AND (NOT $4::bool OR public AND kind='attachment')) AS "owned!""#,
        t,
        id,
        kind,
        public
    )
    .fetch_one(db)
    .await?)
}

/// The current catalogue text of a product with its translations into the enabled languages, in one snapshot.
pub async fn evidence<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
    main_locale: &str,
    locales: &[String],
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT jsonb_build_object(
          'id',p.id,'active',p.active,'locale',$3::text,
          'relationships',jsonb_build_object('parentId',p.parent_id,
            'crossSellingIds',CASE WHEN jsonb_typeof(p.extra->'crossSelling')='array'
              THEN p.extra->'crossSelling' ELSE '[]'::jsonb END),
          'title',coalesce(main.name,p.name),'content',coalesce(main.description,p.description),
          'translations',coalesce((SELECT jsonb_object_agg(l.locale,jsonb_build_object('title',tr.name,'content',tr.description))
           FROM product_translations tr JOIN languages l ON l.id=tr.language_id
           WHERE tr.tenant=p.tenant AND tr.product_id=p.id AND l.locale=ANY($4)
             AND l.locale<>$3 AND (tr.name IS NOT NULL OR tr.description IS NOT NULL)), '{}'::jsonb)) AS "product!"
         FROM products p
         LEFT JOIN languages lang ON lang.locale=$3
         LEFT JOIN product_translations main ON main.tenant=p.tenant AND main.product_id=p.id AND main.language_id=lang.id
         WHERE p.tenant=$1 AND p.id=$2"#,
        tenant,
        id,
        main_locale,
        locales
    )
    .fetch_optional(db)
    .await?)
}
