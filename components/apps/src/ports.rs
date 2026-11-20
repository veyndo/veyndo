//! What the app platform needs from the server: the shared state, its own execution limits and the record rules of bundled apps.
use crate::service::limits::ServiceLimits;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, State};

/// The pending check of a record against the rules of its bundled app.
pub type RuleFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// Rules a bundled app adds to its own managed records; every write passes them inside the write transaction.
pub trait RecordRules: Send + Sync {
    /// The stored form of a record input; apps without their own form return it unchanged.
    fn normalize(&self, m: &Manifest, v: &Value) -> Result<Value>;
    /// Checks a normalized record input before it is written.
    fn validate<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        m: &'a Manifest,
        e: &'a Entity,
        v: &'a Value,
    ) -> RuleFuture<'a>;
}

/// The state of the app platform; it dereferences to the shared database and HTTP state.
#[derive(Clone)]
pub struct Platform {
    pub core: State,
    pub limits: Arc<ServiceLimits>,
    pub rules: &'static dyn RecordRules,
}
impl std::ops::Deref for Platform {
    type Target = State;
    fn deref(&self) -> &State {
        &self.core
    }
}
