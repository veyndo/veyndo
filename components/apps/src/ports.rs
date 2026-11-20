//! What the app platform needs from the server: the shared state, its own execution limits, the record rules of bundled apps and the commerce side of an installation.
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

/// What installing or switching an app needs from commerce, inside the package transaction.
pub trait CommerceInstall: Send + Sync {
    /// Takes the shop configuration lock, so that installs and settings writes keep one order.
    fn lock<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
    ) -> RuleFuture<'a>;
    /// Checks that flows survive a replaced package and installs the payment methods it brings.
    fn install<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        m: &'a Manifest,
    ) -> RuleFuture<'a>;
}

/// The state of the app platform; it dereferences to the shared database and HTTP state.
#[derive(Clone)]
pub struct Platform {
    pub core: State,
    pub limits: Arc<ServiceLimits>,
    pub rules: &'static dyn RecordRules,
    pub commerce: &'static dyn CommerceInstall,
}
impl std::ops::Deref for Platform {
    type Target = State;
    fn deref(&self) -> &State {
        &self.core
    }
}
