//! Provider protocol adapters and prompt evidence; no commerce write capability.
pub use veyndo_runtime::{http_json, scoped_pool, tenant_scope};
pub mod images;
pub mod inference;
pub mod ports;
pub mod prompts;
