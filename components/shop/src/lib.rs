//! Shop component: the settings every other commerce component reads, with their bounds and inheritance rules.
pub mod domain;
pub mod ports;
pub mod types;
pub use types::settings::Settings;
pub mod http;
pub mod service;
pub mod store;
