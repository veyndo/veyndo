//! What the site builder needs from the server: the shared state, the model context, the catalogue page, proposal application, the app surface registry, design reference capture and the coding-agent connection.
use serde_json::Value;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::{PortFuture, RequestContext, Result, State};
use veyndo_intelligence::service::models::Models;

pub trait SiteHost: Send + Sync {
    /// Database pool, HTTP client and admission.
    fn core(&self) -> &State;
    /// The model context: state, providers and inference slots.
    fn models(&self) -> Models<'_>;
    /// One page of the shop catalogue for the page context.
    fn catalog<'a>(
        &'a self,
        h: &'a RequestContext,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value>;
    /// Applies an approved proposal through the native review path.
    fn apply<'a>(
        &'a self,
        tenant: &'a str,
        id: &'a str,
        h: &'a RequestContext,
    ) -> PortFuture<'a, Value>;
    /// The app surfaces of the shop in its languages; `public` keeps storefront surfaces only.
    fn surfaces<'a>(&'a self, h: &'a RequestContext, public: bool) -> PortFuture<'a, Value>;
    /// Bounded design references (public pages and images) as model evidence.
    fn references<'a>(&'a self, v: &'a Value) -> PortFuture<'a, (Value, Vec<String>)>;
    /// The credential-free local coding-agent connection of the shop.
    fn agent_connection(&self, tenant: &str) -> Result<Value>;
}
