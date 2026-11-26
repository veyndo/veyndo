//! The Vercel app: deploy hook triggers.
use serde_json::{Value, json};
use veyndo_connector_runtime::{Call, Error, Result, checked, network, secret};
pub mod provider;
