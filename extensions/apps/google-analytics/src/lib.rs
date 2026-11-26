//! The Google Analytics app: its OAuth policy and settings, property actions and the report source import.
use serde_json::{Value, json};
use veyndo_connector_runtime::{
    Error, OAuth, Result, Settings, Store, checked, network, oauth, text,
};
pub mod actions;
pub mod provider;
