//! The Contentful app: paged entry reads imported as private knowledge sources.
use serde_json::{Value, json};
use veyndo_connector_runtime::{
    Call, Error, Result, bearer, checked, digest, identifier, network, secret, text,
};
pub mod provider;
