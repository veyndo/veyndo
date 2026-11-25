//! Settings, company logos and overrides of a sandbox shop: copying, locked snapshot reads and release writes, inside the caller's transaction.
use serde_json::Value;
use veyndo_core::Result;

/// Locks the settings row first, in the same order as native organization writes.
pub async fn lock_settings(c: &mut sqlx::PgConnection, t: &str) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut *c)
    .await?;
    Ok(())
}

/// The settings document, locked.
pub async fn locked_settings(c: &mut sqlx::PgConnection, t: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut *c)
    .await?)
}

/// The live settings document.
pub async fn settings(c: &mut sqlx::PgConnection, t: &str) -> Result<Value> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM commerce_settings WHERE tenant=$1", t)
            .fetch_one(&mut *c)
            .await?,
    )
}

/// Whether a channel has its own settings overrides.
pub async fn channel_configured(
    c: &mut sqlx::PgConnection,
    t: &str,
    channel: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2) AS "exists!""#, t, channel)
    .fetch_one(&mut *c)
    .await?)
}

/// A channel's settings overrides.
pub async fn channel_overrides(
    c: &mut sqlx::PgConnection,
    t: &str,
    channel: &str,
) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2",
        t,
        channel
    )
    .fetch_one(&mut *c)
    .await?)
}

/// Copies the live settings document into the sandbox.
pub async fn copy_settings(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_settings(tenant,data) SELECT $2,data FROM commerce_settings WHERE tenant=$1",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Removes app payment methods from the sandbox settings; no external services or PSP configurations are registered for a sandbox.
pub async fn drop_app_payments(c: &mut sqlx::PgConnection, id: &str) -> Result<()> {
    sqlx::query!("UPDATE commerce_settings SET data=jsonb_set(data,'{payments}',COALESCE((SELECT jsonb_agg(p) FROM jsonb_array_elements(data->'payments') p WHERE p->>'mode'<>'app'),'[]')) WHERE tenant=$1", id).execute(&mut *c).await?;
    Ok(())
}

/// Copies company logos, company overrides and channel settings overrides into the sandbox.
pub async fn copy_company_and_overrides(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<()> {
    sqlx::query!("INSERT INTO company_logos(tenant,id,content,digest,mime,width,height) SELECT $2,id,content,digest,mime,width,height FROM company_logos WHERE tenant=$1", t, id).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO company_overrides(tenant,channel_id,data) SELECT $2,channel_id,data FROM company_overrides WHERE tenant=$1", t, id).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO commerce_overrides(tenant,channel_id,data) SELECT $2,channel_id,data FROM commerce_overrides WHERE tenant=$1", t, id).execute(&mut *c).await?;
    Ok(())
}

/// Replaces the settings document at the next revision.
pub async fn replace_settings(c: &mut sqlx::PgConnection, t: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE commerce_settings SET data=$1,revision=revision+1 WHERE tenant=$2",
        data,
        t
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}
