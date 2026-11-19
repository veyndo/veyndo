//! Conversions from library errors into the shared error contract; the wire messages are part of the public API.
use crate::{Error, StatusCode, conflict};

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        if matches!(e, sqlx::Error::PoolTimedOut) {
            return Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Database capacity busy; retry later".into(),
            );
        }
        if let sqlx::Error::Database(ref d) = e
            && d.constraint()
                .is_some_and(|c| c.starts_with("app_storage_usage_") && c.ends_with("_check"))
        {
            return Error(StatusCode::PAYLOAD_TOO_LARGE,"App storage quota exceeded (100,000 records or 64 MiB); remove data or contact the operator".into());
        }
        if let sqlx::Error::Database(ref d) = e
            && d.code().as_deref() == Some("23505")
            && d.constraint()
                .is_some_and(|c| c.starts_with("idx_") || c == "app_record_unique")
        {
            return conflict("App field value must be unique; another record already uses it");
        }
        if let sqlx::Error::Database(ref d) = e
            && d.code().as_deref() == Some("23503")
            && d.table().is_some_and(|t| t.starts_with("app_"))
        {
            return conflict(
                "App relationship is missing or still used by another record; check the selected records before retrying",
            );
        }
        eprintln!("database: {e}");
        Error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database operation failed".into(),
        )
    }
}
