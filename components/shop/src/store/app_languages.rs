//! The language members of the shop settings that app lifecycles and extension projections read.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// The stored main language of the shop; `None` when the shop or the member is missing.
pub async fn main_locale(conn: &mut PgConnection, t: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT data->>'mainLocale' AS main FROM commerce_settings WHERE tenant=$1",
        t
    )
    .fetch_optional(conn)
    .await?
    .flatten())
}

/// The main language and enabled languages of the shop as stored: `{"mainLocale":…,"locales":…}`.
pub async fn languages(conn: &mut PgConnection, t: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('mainLocale',data->'mainLocale','locales',data->'locales') AS "language!" FROM commerce_settings WHERE tenant=$1"#,
        t
    )
    .fetch_one(conn)
    .await?)
}
