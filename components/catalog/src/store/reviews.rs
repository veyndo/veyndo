//! Stored product reviews: one per product and reviewer, published only by moderation.
use crate::types::review::{ImportedReview, NewReview};
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

pub async fn product_exists(a: &State, tenant: &str, product: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)")
            .bind(tenant)
            .bind(product)
            .fetch_one(&a.db)
            .await?,
    )
}

pub async fn product_exists_in(tx: &mut PgConnection, tenant: &str, product: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)")
            .bind(tenant)
            .bind(product)
            .fetch_one(tx)
            .await?,
    )
}

/// Stores an unpublished review; `false` when this reviewer already reviewed the product.
pub async fn insert(
    a: &State,
    tenant: &str,
    id: &str,
    product: &str,
    reviewer: &str,
    review: &NewReview,
    verified: bool,
) -> Result<bool> {
    Ok(sqlx::query("INSERT INTO product_reviews(id,tenant,product_id,session,author,rating,title,content,verified) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(tenant,product_id,session) DO NOTHING")
        .bind(id)
        .bind(tenant)
        .bind(product)
        .bind(reviewer)
        .bind(&review.author)
        .bind(review.rating)
        .bind(&review.title)
        .bind(&review.content)
        .bind(verified)
        .execute(&a.db)
        .await?
        .rows_affected()
        == 1)
}

/// Publishes or hides a review; `false` when the shop has no such review.
pub async fn set_approved(a: &State, tenant: &str, id: &str, approved: bool) -> Result<bool> {
    Ok(
        sqlx::query("UPDATE product_reviews SET approved=$1 WHERE tenant=$2 AND id=$3")
            .bind(approved)
            .bind(tenant)
            .bind(id)
            .execute(&a.db)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Serializes imports of one review within the transaction.
pub async fn lock_import(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,28))")
        .bind(format!("{tenant}:review:{id}"))
        .execute(tx)
        .await?;
    Ok(())
}

/// The source record a review was imported from, locked; `None` when the review does not exist or is native.
pub async fn imported_source(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar(
        "SELECT source_import FROM product_reviews WHERE tenant=$1 AND id=$2 FOR UPDATE",
    )
    .bind(tenant)
    .bind(id)
    .fetch_optional(tx)
    .await?
    .flatten())
}

pub async fn insert_imported(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    product: &str,
    review: &ImportedReview<'_>,
    source: &Value,
) -> Result<()> {
    sqlx::query("INSERT INTO product_reviews(id,tenant,product_id,session,author,rating,title,content,verified,approved,created_at,source_import) VALUES($1,$2,$3,$4,$5,$6,$7,$8,false,false,$9::text::timestamptz,$10)")
        .bind(id)
        .bind(tenant)
        .bind(product)
        .bind(format!("import:{id}"))
        .bind(review.author)
        .bind(review.rating)
        .bind(review.title)
        .bind(review.content)
        .bind(review.created.to_rfc3339())
        .bind(source)
        .execute(tx)
        .await?;
    Ok(())
}
