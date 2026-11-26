//! The process-owned identity state: the legacy instance bootstrap credential, empty when personal-only hosting disables it.
use std::sync::Arc;

/// Cloning shares the same credential.
#[derive(Clone, Default)]
pub struct IdentityState {
    /// The configured `MERCHANT_TOKEN`, or empty.
    pub bootstrap_token: Arc<String>,
}
