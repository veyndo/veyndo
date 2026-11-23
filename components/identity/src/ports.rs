//! What role management needs from the server: the shop languages role names are written in.
use serde_json::Value;
use veyndo_core::PortFuture;

/// The content languages of a shop, read from its settings.
pub trait RoleLanguages: Send + Sync {
    /// The main and the enabled content languages.
    fn languages<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, (String, Vec<String>)>;
    /// Checks localized role names under the settings lock the role transaction holds until it commits.
    fn check_names<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        names: &'a Value,
    ) -> PortFuture<'a, ()>;
}
