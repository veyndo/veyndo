//! Existing aggregate app row/byte budget; shared record triggers maintain it without per-app DDL or recounts.
use veyndo_app_model::manifest::Manifest;
use veyndo_core::Result;

/// Gives an installed app its storage budget row.
pub async fn attach(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
) -> Result<()> {
    sqlx::query("INSERT INTO app_storage_usage(tenant,app) VALUES($1,$2) ON CONFLICT DO NOTHING")
        .bind(t)
        .bind(&m.id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
