//! Lookups in the global language registry for components below the shop.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The ID of a registered locale.
pub async fn id_of(conn: &mut PgConnection, locale: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT id FROM languages WHERE locale=$1", locale)
            .fetch_optional(conn)
            .await?,
    )
}
