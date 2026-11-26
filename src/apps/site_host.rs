//! The server side of the site builder port: shared state, model context, catalogue page, proposal application, surface registry, design references and the coding-agent connection.
use crate::foundation::App;
use axum::extract::{FromRef, State};
use serde_json::Value;
use std::sync::Arc;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::request_context::RequestContext;
use veyndo_core::{PortFuture, Result};
use veyndo_intelligence::service::models::Models;
use veyndo_site_builder::{SiteHost, SiteState};

/// The site builder routes take their port from the server state.
impl FromRef<App> for SiteState {
    fn from_ref(app: &App) -> Self {
        SiteState(Arc::new(Site(app.clone())))
    }
}

/// The owners of this process the site builder reaches.
pub(crate) struct Site(pub(crate) App);
impl SiteHost for Site {
    fn core(&self) -> &veyndo_core::State {
        &self.0.core
    }
    fn models(&self) -> Models<'_> {
        self.0.models()
    }
    fn catalog<'a>(
        &'a self,
        h: &'a RequestContext,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(
                crate::catalog_routes::catalog_page(State(self.0.clone()), h.clone(), criteria)
                    .await?
                    .0,
            )
        })
    }
    fn apply<'a>(
        &'a self,
        tenant: &'a str,
        id: &'a str,
        h: &'a RequestContext,
    ) -> PortFuture<'a, Value> {
        Box::pin(crate::proposal_apply::apply(&self.0, tenant, id, h))
    }
    fn surfaces<'a>(&'a self, h: &'a RequestContext, public: bool) -> PortFuture<'a, Value> {
        Box::pin(crate::apps::surfaces::registry(&self.0, h, public))
    }
    fn references<'a>(&'a self, v: &'a Value) -> PortFuture<'a, (Value, Vec<String>)> {
        Box::pin(crate::assets::design_references::capture(v))
    }
    fn agent_connection(&self, tenant: &str) -> Result<Value> {
        crate::developer::agent_connection::configuration(tenant)
    }
}
