//! Whether a hosted frontend alias is bound to a shop's sales channel, read on the caller's connection.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The alias is a hosted frontend of this shop and channel.
pub async fn bound(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
    alias: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND channel=$2 AND alias=$3) AS "exists!""#,
        tenant,
        channel,
        alias
    )
    .fetch_one(conn)
    .await?)
}
