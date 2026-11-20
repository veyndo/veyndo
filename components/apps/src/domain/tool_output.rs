//! Declared tool results are checked before they leave; a provider payload that does not fit is never disclosed.
use crate::domain::input_schema;
use serde_json::Value;
use veyndo_app_model::Action;
use veyndo_core::{Error, Result};

/// Returns the result only if it fits the output schema the action declares.
pub fn checked_output(action: &Action, result: Value) -> Result<Value> {
    if let Some(schema) = &action.output_schema {
        input_schema::validate_value(schema, &result).map_err(|_| Error::output_contract())?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::checked_output;
    use crate::domain::approval::built_in;
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn a_result_outside_the_declared_schema_is_withheld() {
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        let mut action = m.actions[0].clone();
        action.output_schema = None;
        assert_eq!(
            checked_output(&action, json!({"any":1})).ok(),
            Some(json!({"any":1}))
        );
        action.output_schema = Some(
            json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}),
        );
        assert!(checked_output(&action, json!({"ok":true})).is_ok());
        assert!(checked_output(&action, json!({"secret":"payload"})).is_err());
    }
}
