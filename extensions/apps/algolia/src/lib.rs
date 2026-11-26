//! The Algolia app: bounded, verified index record writes.
use serde_json::{Value, json};
use veyndo_connector_runtime::{Call, Error, Result, checked, identifier, network, secret, short};
pub mod provider;
