//! The starter Wasm business-policy extension of a shop.
use veyndo_core::Result;

/// The shop's starter Wasm extension; `keep_existing` leaves a present one untouched.
pub async fn insert_extension<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    wat: &str,
    digest: &str,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!(
            "INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
            slug,
            wat,
            digest
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3)",
            slug,
            wat,
            digest
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// The saved Wasm extension of a shop, if any.
pub async fn extension<'e>(db: impl sqlx::PgExecutor<'e>, slug: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT wat FROM extensions WHERE tenant=$1", slug)
            .fetch_optional(db)
            .await?,
    )
}
