//! Versioned app packages: managed data, UI slots, agent tools and isolated service calls.
mod approval;
mod record_import;
pub(crate) use record_import::{import_record, inspect_import_record};
mod asset_surfaces;
mod cart_contributions;
pub(crate) use veyndo_app_platform::service::commerce_hooks::Prepared as PreparedCheckoutHooks;
pub(crate) use veyndo_app_platform::service::commerce_hooks::{
    apply as apply_hooks, prepare as prepare_hooks,
};
mod configuration_events;
pub(crate) use configuration_events::{configuration_budget, configuration_events};
pub(crate) use veyndo_app_platform::store::configuration_orders::configurations;
mod configuration_selection;
pub(crate) use configuration_selection::{
    Selection as ConfigurationSelection, resolve_selection as configuration_selection_for_list,
    resolve_selections,
};
mod consent;
mod core_callbacks;
pub(crate) mod core_extensions;
mod install_host;
mod package_fence;
pub(crate) use package_fence::lock_package_write;
mod core_documents;
mod core_exchange;
mod credentials;
pub(crate) use credentials::admit_catalog_sync;
pub(crate) mod discovery;
mod distribution;
pub(crate) use distribution::sign_package;
pub(crate) mod data;
mod events;
mod schedules;
mod secrets;
mod settings;
mod webhook_scope;
mod webhooks;
pub(crate) use schedules::schedule_once;
pub(crate) use webhook_scope::bind as bind_webhook_scope;
mod evidence;
mod evidence_routes;
pub(crate) use evidence::*;
mod egress;
// Shared bounded public HTTPS transport; protocols must never forward customer credentials.
pub(crate) use egress::public_client as public_https_client;
mod gateway;
pub(crate) mod mcp_views;
pub(crate) use mcp_views::{invoke_mcp, invoke_mcp_read};
mod tool_contract;
pub(crate) use tool_contract::{app_tools, app_tools_for};
mod input_schema;
mod job_import;
mod jobs;
pub(crate) mod native_lifecycle;
pub(crate) mod record_history;
pub(crate) use approval::print_digest;
pub(crate) mod hosted;
pub(crate) mod service_limits;
mod service_policy;
mod surface_grants;
mod surface_registry;
pub(crate) mod surfaces;
mod ui_bundles;
mod ui_documents;
pub(crate) use ui_documents::SandboxedAppDocument;
mod observability;
mod planning;
pub(crate) use planning::*;
mod manifest;
mod manifest_validation;
pub(crate) mod site_host;
pub(crate) use veyndo_site_builder as site_builder;
use veyndo_site_forms as site_forms;
mod submissions;
pub(crate) use manifest_validation::validate;
pub(crate) mod model_contract;
mod registry;
mod routes;
pub(crate) use cart_contributions::*;
pub(crate) use events::*;
pub(crate) use gateway::{invoke_app, validate_native_input};
pub(crate) use manifest::*;
pub(crate) use registry::*;
pub(crate) use routes::*;

pub(crate) use service_policy::allowed as service_origin_allowed;
