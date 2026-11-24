//! The server side of the release port: app installation and managed record saves with the bundled rules, flow checks, settings guards and the sandbox policy cache.
use crate::foundation::App;
use crate::{apps, commerce, marketing};
use serde_json::Value;
use std::sync::Arc;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::PortFuture;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_studio::ports::ReleaseHost;

/// The server lends release work its app and flow owners.
pub(crate) struct Releases<'a>(pub(crate) &'a App);
impl ReleaseHost for Releases<'_> {
    fn core(&self) -> &veyndo_core::State {
        &self.0.core
    }
    fn settings_guards(&self) -> &dyn veyndo_shop::ports::SettingsGuards {
        &commerce::Guards
    }
    fn install_app<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        manifest: Manifest,
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::install_tx(tx, t, manifest))
    }
    fn save_record<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        manifest: &'a Manifest,
        entity: &'a Entity,
        value: &'a Value,
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::data::save_tx(tx, t, manifest, entity, value))
    }
    fn validate_flow<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        h: &'a RequestContext,
        value: &'a Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(marketing::validate_flow_config(self.0, tx, t, h, value))
    }
    fn authorize_flow(&self, h: &RequestContext, flow: &Value) -> Result<()> {
        let flow: marketing::Flow =
            serde_json::from_value(flow.clone()).map_err(|_| bad("Invalid flow"))?;
        marketing::authorize_flow(h, &flow)
    }
    fn register_sandbox(&self, id: &str, wat: &str) -> Result<()> {
        self.0
            .sandboxes
            .insert(id.into(), Arc::new(Sandbox::new(wat).map_err(bad)?));
        Ok(())
    }
}
