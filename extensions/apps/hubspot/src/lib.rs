//! The HubSpot app: verified contact upserts.
use serde_json::{Value, json};
use veyndo_connector_runtime::{Call, Result, bearer, checked, network, secret, short};
use veyndo_provider_email::config;
pub mod provider;
