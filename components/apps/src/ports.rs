//! What the app platform needs from the server: the shared state, its own execution limits, the record rules of bundled apps, the commerce side of an installation and the action handlers the server owns.
use crate::service::limits::ServiceLimits;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::{Action, Entity};
use veyndo_core::{RequestContext, Result, State};

/// The pending check of a record against the rules of its bundled app.
pub type RuleFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// Rules a bundled app adds to its own managed records; every write passes them inside the write transaction.
pub trait RecordRules: Send + Sync {
    /// The stored form of a record input; apps without their own form return it unchanged.
    fn normalize(&self, m: &Manifest, v: &Value) -> Result<Value>;
    /// Checks record fields against the app's own shape before a proposal is bound; it reads nothing.
    fn check(&self, m: &Manifest, e: &Entity, fields: &Value) -> Result<()>;
    /// The nested format a planner must follow for the records of this app; null without one.
    fn planning_format(&self, m: &Manifest) -> Value;
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

/// The pending result of an action the server runs itself.
pub type ActionFuture<'a> = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;
/// The pending local answer of a connected hosted app, if it has one.
pub type HostedFuture<'a> = Pin<Box<dyn Future<Output = Result<Option<Value>>> + Send + 'a>>;

/// The pending shop id of a newly created environment.
pub type EnvironmentFuture<'a> = Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

/// One admitted app action whose handler the server owns.
pub struct HostCall<'a> {
    pub actor: &'a RequestContext,
    pub tenant: &'a str,
    pub manifest: &'a Manifest,
    pub action: &'a Action,
    pub entity: Option<&'a Entity>,
    pub input: &'a Value,
}

/// Action handlers that touch carts, files, payments or search stay with the server; the gateway admits the call first.
pub trait HostActions: Send + Sync {
    /// The local answer of a connected hosted app; `None` lets the declared handler run.
    fn hosted<'a>(&'a self, tenant: &'a str, app: &'a str, action: &'a str) -> HostedFuture<'a>;
    /// Runs a handler the platform does not own.
    fn run<'a>(&'a self, call: HostCall<'a>) -> ActionFuture<'a>;
    /// Admits an object a storefront visitor names in a surface context: their own customer or unit, a visible product or the current sales channel.
    fn visitor_object<'a>(
        &'a self,
        h: &'a RequestContext,
        key: &'a str,
        id: &'a str,
    ) -> RuleFuture<'a>;
    /// Removes the imported sources of an app after its service asked for it.
    fn purge_sources<'a>(&'a self, tenant: &'a str, app: &'a str, cursor: i64) -> RuleFuture<'a>;
    /// Clones the live shop into a personal preview environment of the actor and returns its shop id.
    fn create_preview<'a>(&'a self, h: &'a RequestContext, actor: &'a str)
    -> EnvironmentFuture<'a>;
}

/// The state of the app platform; it dereferences to the shared database and HTTP state.
#[derive(Clone)]
pub struct Platform {
    pub core: State,
    pub limits: Arc<ServiceLimits>,
    pub rules: &'static dyn RecordRules,
    pub commerce: &'static dyn CommerceInstall,
    pub actions: Arc<dyn HostActions>,
}
impl std::ops::Deref for Platform {
    type Target = State;
    fn deref(&self) -> &State {
        &self.core
    }
}
