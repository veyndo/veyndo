//! Which commerce hook packages a shop admits and checkout evaluates.
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad};

/// The active commerce hook packages of a shop, locked against upgrades; at most eight are admitted.
pub async fn rows(conn: &mut sqlx::PgConnection, t: &str) -> Result<Vec<serde_json::Value>> {
    // Row locks fence upgrades/deactivation during checkout. Nine detects the eight-app admission cap.
    let rows = sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND active AND manifest ? 'commerceHooks' ORDER BY id LIMIT 9 FOR SHARE",
        t
    )
    .fetch_all(conn)
    .await?;
    if rows.len() > 8 {
        return Err(bad("At most eight active commerce-hook apps are admitted"));
    }
    Ok(rows)
}
/// Admits a commerce hook package only below the cap of eight per shop.
pub async fn admission(conn: &mut sqlx::PgConnection, t: &str, m: &Manifest) -> Result<()> {
    if m.commerce_hooks.is_some() {
        let count = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM app_packages WHERE tenant=$1 AND active AND id<>$2 AND manifest ? 'commerceHooks'"#,
            t,
            m.id
        )
        .fetch_one(conn)
        .await?;
        if count >= 8 {
            return Err(bad("At most eight active commerce-hook apps are admitted"));
        }
    }
    Ok(())
}
