//! The consent change log and legal acknowledgements of carts: appended entries, the daily change count and the stored acknowledgement.
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// Consent changes of a receipt during the last day.
pub async fn changes_today(e: impl PgExecutor<'_>, tenant: &str, owner: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM privacy_consent_log WHERE tenant=$1 AND cart_id=$2 AND created_at>now()-interval '1 day'"#,
        tenant,
        owner
    )
    .fetch_one(e)
    .await?)
}

/// Appends an entry to the consent log.
pub async fn log(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    channel: &str,
    entry: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO privacy_consent_log(tenant,cart_id,channel_id,data) VALUES($1,$2,$3,$4)",
        tenant,
        cart,
        channel,
        entry
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Records the cart's legal acknowledgement, replacing an earlier one.
pub async fn accept_legal(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO legal_acceptances(tenant,cart_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,cart_id) DO UPDATE SET data=EXCLUDED.data",
        tenant, cart, data).execute(e).await?;
    Ok(())
}
