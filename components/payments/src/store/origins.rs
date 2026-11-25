//! Whether a browser origin's alias is a hosted frontend of the shop's channel. Checked at compile time.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The alias is bound to the channel's hosted frontend of the shop.
pub async fn hosted_alias(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
    alias: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND channel=$2 AND alias=$3) AS "owned!""#,
        tenant,
        channel,
        alias
    )
    .fetch_one(conn)
    .await?)
}
