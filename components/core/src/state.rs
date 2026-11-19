//! The state every component may use: the tenant-scoped database pool, the outbound HTTP client and request admission.
use veyndo_runtime::scoped_pool::ScopedPool;

/// A component function takes `&State`; the server's own state dereferences to it, so call sites pass theirs unchanged.
#[derive(Clone)]
pub struct State {
    pub db: ScopedPool,
    pub http: reqwest::Client,
    pub admission: std::sync::Arc<crate::admission::Admission>,
}
