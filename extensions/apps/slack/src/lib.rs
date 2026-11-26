//! The Slack app: its OAuth policy and settings, channel actions and order notifications.
use serde_json::{Value, json};
use veyndo_connector_runtime::{
    Error, OAuth, Result, Settings, Store, checked, network, oauth, text,
};
use veyndo_provider_email::templates;
pub mod actions;
pub mod provider;
