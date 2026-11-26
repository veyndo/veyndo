//! Provider-independent payment ledger and durable workers; the PayPal adapter supports explicit Sandbox/Live environments.

mod accounts;
mod app_commands;
pub(crate) mod host;
#[cfg(test)]
mod host_contract_tests;
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
mod provider;
mod provider_webhooks;
pub(crate) mod remote;
mod routes;
pub(crate) mod sessions;
mod storage;
mod worker;
pub(crate) use operations::*;
pub(crate) use provider::*;
pub(crate) use routes::*;
pub(crate) use worker::*;
