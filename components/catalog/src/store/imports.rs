//! Native catalogue state that reviewed migration imports replay against and compare with: categories, product fields, assets and reviews.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A category's data document.
pub async fn category_data(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM categories WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// A category as `{parentId,data}`.
pub async fn category(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('parentId',parent_id,'data',data) AS "value!" FROM categories WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// A product's `extra` document.
pub async fn product_extra(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT extra FROM products WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// A product's media list.
pub async fn product_media(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT media FROM products WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// An asset's metadata with the digest and length of its content.
pub async fn asset(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('productId',product_id,'digest',encode(sha256(content),'hex'),'byteLength',octet_length(content),'mime',mime,'filename',filename,'title',title,'public',public) AS "value!" FROM product_assets WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}

/// A review in its import shape.
pub async fn review(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('productId',product_id,'sourceAuthor',author,'rating',rating,'title',title,'text',content,'verifiedPurchase',verified,'sourceDigest',source_import->>'recordDigest','createdAt',to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) AS "value!" FROM product_reviews WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?)
}
