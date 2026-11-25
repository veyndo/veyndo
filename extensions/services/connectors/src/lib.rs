//! Language-neutral app HTTP contract backed by a Rust-only standard connector runtime: the server, the worker, the app action and event dispatch and the app registries over the provider crates.
mod actions;
mod events;
mod integrations;
mod providers;
mod server;
#[cfg(test)]
mod tests;
mod worker;
use serde_json::{Value, json};
pub use server::run;
use std::{env, time::Duration};
use veyndo_connector_runtime::{
    Call, Error, OAuth, Result, Settings, Store, checked, crypto, identifier, network, oauth,
    queue, secret, text,
};
use veyndo_provider_email::{config, email};
const APPS: [&str; 9] = [
    "email",
    "gmail",
    "google_analytics",
    "slack",
    "vercel",
    "klaviyo",
    "hubspot",
    "contentful",
    "algolia",
];

#[cfg(test)]
mod integration_contracts;
