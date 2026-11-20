//! Native API/MCP error adapter for the independent bounded schema validator.
use crate::domain::model_contract;
use serde_json::Value;
use veyndo_core::Result;

/// Checks an input object against a declared schema.
pub fn validate(schema: &Value, value: &Value) -> Result<()> {
    veyndo_app_model::schema::validate(schema, value).map_err(model_contract::error)
}
/// Checks an input object against a schema of the native tool catalogue.
pub fn validate_native(schema: &Value, value: &Value) -> Result<()> {
    veyndo_app_model::schema::validate_native(schema, value).map_err(model_contract::error)
}
/// Checks any value, not only an object, against a declared schema.
pub fn validate_value(schema: &Value, value: &Value) -> Result<()> {
    veyndo_app_model::schema::validate_value(schema, value).map_err(model_contract::error)
}
#[cfg(test)]
#[path = "input_schema_tests.rs"]
mod tests;
