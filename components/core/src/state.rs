//! The state every component may use: the tenant-scoped database pool, the outbound HTTP client and request admission.
use veyndo_runtime::scoped_pool::ScopedPool;

/// A component function takes `&State`; the server's own state dereferences to it, so call sites pass theirs unchanged.
#[derive(Clone)]
pub struct State {
    pub db: ScopedPool,
    pub http: reqwest::Client,
    pub admission: std::sync::Arc<crate::admission::Admission>,
    /// Per-channel request counters this process flushes periodically.
    pub metrics: std::sync::Arc<crate::channel_metrics::ChannelMetrics>,
}

impl State {
    /// A state whose database is never reached, for tests of code that works through ports only.
    pub fn detached() -> Self {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://detached.invalid/none")
            .expect("lazy pool");
        Self {
            db: ScopedPool::new(pool, false),
            http: reqwest::Client::new(),
            admission: std::sync::Arc::new(crate::admission::Admission::default()),
            metrics: std::sync::Arc::new(crate::channel_metrics::ChannelMetrics::default()),
        }
    }
}
