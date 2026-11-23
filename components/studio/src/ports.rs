//! What reviewed work and proposal approval need from the server: the native operation registry, the app gateway's managed saves and the catalogue state.
use serde_json::Value;
use veyndo_app_platform::domain::planning::AppChange;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_intelligence::ports::AgentHost;

/// The server side of reviewed work; binding, approval, execution and application stay in this component.
pub trait WorkHost: AgentHost {
    /// The native configuration operations the actor may run as reviewed work, with their input schemas.
    fn work_operations(&self, h: &RequestContext) -> Vec<(&'static str, Value)>;
    /// Binds an app change to the installed app's current revision and managed save contract.
    fn bind_change<'a>(
        &'a self,
        t: &'a str,
        change: &'a mut AppChange,
        h: &'a RequestContext,
    ) -> PortFuture<'a, ()>;
    /// Applies a bound app change inside the caller's transaction.
    fn apply_change<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        change: &'a AppChange,
        h: &'a RequestContext,
    ) -> PortFuture<'a, ()>;
    /// The catalogue state with its ports.
    fn catalog(&self) -> veyndo_catalog::ports::Catalog;
}

/// What sandbox clone, snapshot and release need from the server: app installation and managed record saves with the bundled rules, flow checks, the settings guards and agent-run-free release authority.
pub trait ReleaseHost: Send + Sync {
    /// The shared state.
    fn core(&self) -> &veyndo_core::State;
    /// The cross-component checks of a settings write.
    fn settings_guards(&self) -> &dyn veyndo_shop::ports::SettingsGuards;
    /// Installs an app package inside the caller's transaction.
    fn install_app<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        manifest: veyndo_app_model::manifest::Manifest,
    ) -> PortFuture<'a, Value>;
    /// Saves a managed app record with the bundled record rules.
    fn save_record<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        manifest: &'a veyndo_app_model::manifest::Manifest,
        entity: &'a veyndo_app_model::Entity,
        value: &'a Value,
    ) -> PortFuture<'a, Value>;
    /// Checks a flow configuration against the final release aggregate.
    fn validate_flow<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        h: &'a RequestContext,
        value: &'a Value,
    ) -> PortFuture<'a, ()>;
    /// Requires the publishing actor's native action rights for a flow.
    fn authorize_flow(&self, h: &RequestContext, flow: &Value) -> veyndo_core::Result<()>;
    /// Compiles and caches the sandbox policy of a new environment.
    fn register_sandbox(&self, id: &str, wat: &str) -> veyndo_core::Result<()>;
}

/// What source-record imports need from the server: managed app record imports, product image intake, coupon configuration and the catalogue state.
pub trait ImportHost: Send + Sync {
    /// The shared state.
    fn core(&self) -> &veyndo_core::State;
    /// The catalogue state with its ports.
    fn catalog(&self) -> veyndo_catalog::ports::Catalog;
    /// Imports one managed app record into a reviewed destination package.
    fn import_record<'a>(
        &'a self,
        h: &'a RequestContext,
        id: &'a str,
        destination: &'a Value,
        digest: &'a str,
    ) -> PortFuture<'a, Value>;
    /// Compares one managed app record with its destination without writing.
    fn inspect_record<'a>(
        &'a self,
        h: &'a RequestContext,
        id: &'a str,
        destination: &'a Value,
        digest: &'a str,
    ) -> PortFuture<'a, (Value, Option<Value>)>;
    /// Stores an imported product image through the native asset owner.
    fn import_image<'a>(
        &'a self,
        h: &'a RequestContext,
        product: &'a str,
        id: &'a str,
        metadata: &'a Value,
        bytes: Vec<u8>,
    ) -> PortFuture<'a, Value>;
    /// Saves an imported coupon inactive and unreviewed through the native promotion owner.
    fn import_promotion<'a>(
        &'a self,
        h: &'a RequestContext,
        id: &'a str,
        data: &'a Value,
    ) -> PortFuture<'a, Value>;
}
