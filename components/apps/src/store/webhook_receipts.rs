//! Receipts of incoming app webhooks: one outbox event per (app, webhook, sender event ID), replayed on repeats.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The stored receipt of a webhook event: the payload digest and the outbox event it became.
pub struct Receipt {
    pub digest: String,
    pub event_id: i64,
}

/// The receipt of an earlier delivery of the same sender event, if any.
pub async fn find(
    conn: &mut PgConnection,
    t: &str,
    app: &str,
    webhook: &str,
    key: &str,
) -> Result<Option<Receipt>> {
    Ok(sqlx::query_as!(
        Receipt,
        "SELECT digest,event_id FROM app_webhook_receipts WHERE tenant=$1 AND app=$2 AND webhook=$3 AND request_key=$4",
        t,
        app,
        webhook,
        key
    )
    .fetch_optional(conn)
    .await?)
}

/// Records which outbox event a sender event became.
pub async fn record(
    conn: &mut PgConnection,
    t: &str,
    app: &str,
    webhook: &str,
    key: &str,
    receipt: Receipt,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO app_webhook_receipts(tenant,app,webhook,request_key,digest,event_id) VALUES($1,$2,$3,$4,$5,$6)",
        t,
        app,
        webhook,
        key,
        receipt.digest,
        receipt.event_id
    )
    .execute(conn)
    .await?;
    Ok(())
}
