//! The starter storefront experience and experiment policy variants of a new shop.
use serde_json::Value;
use veyndo_core::Result;

/// The starter storefront experience; `keep_existing` leaves a present one untouched.
pub async fn insert_experience<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    data: Value,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!(
            "INSERT INTO experiences(tenant,data) VALUES($1,$2) ON CONFLICT DO NOTHING",
            slug,
            data
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO experiences(tenant,data) VALUES($1,$2)",
            slug,
            data
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// A starter policy variant; `keep_existing` leaves a present one untouched.
pub async fn insert_policy<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    variant: &str,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!(
            "INSERT INTO policy(tenant,variant) VALUES($1,$2) ON CONFLICT DO NOTHING",
            slug,
            variant
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO policy(tenant,variant) VALUES($1,$2)",
            slug,
            variant
        )
        .execute(db)
        .await?;
    }
    Ok(())
}
