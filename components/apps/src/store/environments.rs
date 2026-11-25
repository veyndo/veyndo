//! The shop environments apps preview in: ownership by a live shop and the personal preview of one merchant, read from the core registry; the preview expiry is the studio's.
use sqlx::PgConnection;
use veyndo_core::Result;
pub use veyndo_core::store::previews::{lock_preview, owned, preview_of};

/// Keeps a preview environment for another hour.
pub async fn extend_preview(conn: &mut PgConnection, environment: &str) -> Result<()> {
    crate::ports::directory()?
        .extend_preview(conn, environment)
        .await
}
