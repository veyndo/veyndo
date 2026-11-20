//! Translate storage-port failures into the existing native HTTP/MCP domain errors.
use veyndo_core::{Error, StatusCode, bad, conflict};

/// The native error of a storage failure: invalid input, conflict, quota or database.
pub fn error(value: veyndo_app_storage::Error) -> Error {
    match value {
        veyndo_app_storage::Error::Database(error) => error.into(),
        veyndo_app_storage::Error::Invalid(message) => bad(message),
        veyndo_app_storage::Error::Conflict(message) => conflict(message),
        veyndo_app_storage::Error::Quota(message) => {
            Error(StatusCode::TOO_MANY_REQUESTS, message.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::error;
    use veyndo_app_storage::Error;
    use veyndo_core::StatusCode;

    #[test]
    fn storage_failures_become_the_matching_native_status() {
        let invalid = error(Error::Invalid("Record too large"));
        assert_eq!(
            (invalid.0, invalid.1.as_str()),
            (StatusCode::BAD_REQUEST, "Record too large")
        );
        assert_eq!(
            error(Error::Conflict("Record changed")).0,
            StatusCode::CONFLICT
        );
        let quota = error(Error::Quota("Storage quota reached"));
        assert_eq!(
            (quota.0, quota.1.as_str()),
            (StatusCode::TOO_MANY_REQUESTS, "Storage quota reached")
        );
        assert!(
            error(Error::Database(sqlx::Error::RowNotFound))
                .0
                .is_client_error()
                || error(Error::Database(sqlx::Error::RowNotFound))
                    .0
                    .is_server_error()
        );
    }
}
