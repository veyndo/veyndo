//! What quote requests need from the server: the compiled app lifecycle a new quote is accepted under.
use serde_json::Value;
use veyndo_core::PortFuture;

/// The app lifecycle of quotes; the app platform compiles it from the active packages.
pub trait QuoteLifecycle: Send + Sync {
    /// The lifecycle snapshot a new quote carries, or null when no app declares one; held until the quote commits.
    fn capture<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Value>;
}
