//! The email app: settings and credentials, bounded multilingual templates, SMTP and provider delivery and the order event mapping; the connector runtime owns storage, queue and egress.
use serde_json::{Value, json};
use std::{env, sync::Arc, time::Duration};
use veyndo_connector_runtime::{Error, Result, Store, checked, digest, network, queue, text};
pub mod config;
pub mod email;
pub mod payment_templates;
pub mod smtp;
pub mod templates;
