//! Provider-independent payment ledger and durable workers; the PayPal adapter supports explicit Sandbox/Live environments.

mod accounts;
mod app_commands;
mod host;
pub(crate) mod instruments;
pub(crate) use accounts::*;
pub(crate) use app_commands::{app_command, command_action};
mod catalog;
pub(crate) use catalog::CAPABILITIES;
mod contract;
mod merchant;
pub(crate) use merchant::invoke as merchant_operation;
mod instructions;
pub(crate) use instructions::customer_read as customer_instructions;
pub(crate) use veyndo_payments::service::instructions::{
    for_document as document_instructions, for_flow as flow_instructions,
};
mod details;
mod diagnostics;
pub(crate) mod registry;
pub(crate) use registry::*;
pub(crate) mod capture_configuration;
mod generic_receipts;
mod operations;
#[path = "../../extensions/apps/paypal/src/compatibility.rs"]
mod paypal;
mod provider;
mod provider_webhooks;
pub(crate) mod remote;
mod return_urls;
mod routes;
pub(crate) mod sessions;
mod storage;
#[path = "../../extensions/apps/paypal/src/webhooks.rs"]
mod webhooks;
mod worker;
pub(crate) use operations::*;
pub(crate) use provider::*;
pub(crate) use routes::*;
pub(crate) use worker::*;

mod receipt_guard;
use receipt_guard::receipt_matches;
mod state;
