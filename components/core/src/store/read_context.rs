//! Version stamps of the shared read contexts; a cached registry is current while its stamp matches.
use crate::Result;
use sqlx::PgConnection;

/// The current version of a read-context namespace, as text.
pub async fn version(conn: &mut PgConnection, namespace: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        r#"SELECT version::text AS "version!" FROM read_context_versions WHERE namespace=$1"#,
        namespace
    )
    .fetch_one(conn)
    .await?)
}
