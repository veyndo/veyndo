//! The native error contract of the pure app model: problem kinds, revision conflicts, escaped JSON pointers and the bounded input schema validator.
use crate::{Error, Result, bad, conflict};
use serde_json::Value;

/// The native error of a model error, with its kind and pointer.
pub fn error(error: veyndo_app_model::Error) -> Error {
    match error.kind {
        veyndo_app_model::Kind::Bad => bad(error.message),
        veyndo_app_model::Kind::Conflict => conflict(&error.message),
        veyndo_app_model::Kind::Validation => {
            Error::validation(error.pointer.as_deref().unwrap_or(""), error.message)
        }
    }
}

/// Checks an input object against a declared schema.
pub fn validate(schema: &Value, value: &Value) -> Result<()> {
    veyndo_app_model::schema::validate(schema, value).map_err(error)
}
/// Checks an input object against a schema of the native tool catalogue.
pub fn validate_native(schema: &Value, value: &Value) -> Result<()> {
    veyndo_app_model::schema::validate_native(schema, value).map_err(error)
}
/// Checks any value, not only an object, against a declared schema.
pub fn validate_value(schema: &Value, value: &Value) -> Result<()> {
    veyndo_app_model::schema::validate_value(schema, value).map_err(error)
}
#[cfg(test)]
#[path = "model_contract/schema_tests.rs"]
mod schema_tests;

#[cfg(test)]
mod tests {
    use super::error;
    use crate::StatusCode;
    use veyndo_app_model::{Error, Kind};

    fn model(kind: Kind, pointer: Option<&str>) -> Error {
        Error {
            kind,
            message: "Declared limit exceeded".into(),
            pointer: pointer.map(String::from),
        }
    }

    #[test]
    fn model_errors_keep_their_kind_message_and_pointer() {
        let bad = error(model(Kind::Bad, None));
        assert_eq!(
            (bad.0, bad.1.as_str()),
            (StatusCode::BAD_REQUEST, "Declared limit exceeded")
        );
        assert_eq!(error(model(Kind::Conflict, None)).0, StatusCode::CONFLICT);
        let invalid = error(model(Kind::Validation, Some("/fields/a~1b")));
        assert_eq!(invalid.problem()["pointer"], "/fields/a~1b");
        assert_eq!(
            error(model(Kind::Validation, None)).problem()["pointer"],
            ""
        );
    }
}
