//! Preserve native problem kinds, revision-conflict status and escaped JSON pointers across the pure model boundary.
use veyndo_core::{Error, bad, conflict};

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

#[cfg(test)]
mod tests {
    use super::error;
    use veyndo_app_model::{Error, Kind};
    use veyndo_core::StatusCode;

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
