//! What the flow runner needs from the server: the effect of one pipeline action node, configuration hosting and the records of other components (carts, customers, orders, app packages, work tasks) that rules, flows and channels read or change inside marketing's own transactions.
use crate::store::catalog::ActiveApp;
use crate::types::flow::Flow;
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::OnceLock;
use veyndo_core::{PortFuture, State};
use veyndo_shop::types::cart::StoredCart;

/// One action of a flow step: shop operations, app actions and payment instructions run in the server.
pub trait FlowEffects: Send + Sync {
    /// Runs the action once under its idempotency key and returns its recorded result.
    fn execute<'a>(
        &'a self,
        tenant: &'a str,
        flow: &'a Flow,
        key: &'a str,
        action: &'a str,
        config: &'a Value,
        definition: &'a Value,
    ) -> PortFuture<'a, Value>;
}

/// What saving marketing configuration needs from the server: actor-private work templates, installed app actions and channel setup of other components.
pub trait ConfigurationHost: Send + Sync {
    /// The shared database and caches.
    fn core(&self) -> &veyndo_core::State;
    /// The work template of a flow references completed work the actor may still copy.
    fn validate_template<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        actor: &'a veyndo_core::RequestContext,
        flow: &'a Flow,
    ) -> PortFuture<'a, ()>;
    /// Every app step of a flow names an installed, active action the actor may publish; `lock` fences concurrent installs.
    fn validate_apps<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        actor: &'a veyndo_core::RequestContext,
        flow: &'a Flow,
        lock: bool,
    ) -> PortFuture<'a, ()>;
    /// A newly saved channel of another kind (POS) prepares its own records in the same transaction.
    fn configure_channel<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        id: &'a str,
        data: &'a Value,
        created: bool,
    ) -> PortFuture<'a, ()>;
}

/// The customer fields customer rules read, with calendar age and login distances in days.
pub struct RuleCustomer {
    pub active: bool,
    pub email: String,
    pub group_name: String,
    pub customer_number: String,
    pub profile: Value,
    pub automation: Value,
    pub company: Option<String>,
    pub age: Option<f64>,
    pub first_days: Option<f64>,
    pub last_days: Option<f64>,
}

/// A customer's order count, total amount and days since the last order.
pub struct CustomerOrders {
    pub count: i64,
    pub amount: f64,
    pub days: Option<f64>,
}

/// The application and execution state of a work task a flow job created.
pub struct TaskState {
    pub id: String,
    pub applied: bool,
    pub execution_started: bool,
}

/// Which automation data a revision-bound rule metadata write targets.
#[derive(Clone, Copy)]
pub enum RuleEntity {
    Customer,
    Order,
}

/// Rows of components that depend on marketing or that marketing does not use, read or changed on marketing's connection: carts (checkout), customers and their sessions (customers), orders, their activity and receipts (orders), active app packages (apps) and work tasks (intelligence). The server implements it over the owners' stores and installs it once at startup.
pub trait Records: Send + Sync {
    /// The cart behind `token`, locked for this transaction.
    fn lock_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>>;
    /// Stores the cart document and advances its revision by one.
    fn save_cart_next_revision<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        data: &'a Value,
        record: (&'a str, &'a str),
    ) -> PortFuture<'a, ()>;
    /// An order with the cart it was placed from; none for an unknown order or a missing cart.
    fn order_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        order: &'a str,
    ) -> PortFuture<'a, Option<(StoredCart, Value)>>;
    /// Signs every open cart of the customer out with fresh tokens derived from `seed`.
    fn sign_out_carts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        seed: &'a str,
        tenant: &'a str,
        customer: Option<&'a str>,
    ) -> PortFuture<'a, ()>;
    /// The rule facts of customer `id`; none for a guest or an unknown id.
    fn rule_customer<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Option<RuleCustomer>>;
    /// The group of customer `id`, locking the customer row.
    fn lock_customer_group<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, String>;
    /// The automation data of customer `id`, locking the customer row; none for an unknown id.
    fn lock_customer_automation<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Option<Value>>;
    /// Changes the group and/or active flag of customer `id`; the number of rows changed.
    fn set_customer_group_and_status<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        change: (Option<&'a str>, Option<bool>),
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, u64>;
    /// Ends every session of customer `id`.
    fn end_customer_sessions<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, ()>;
    /// Stores the automation data of customer `id`.
    fn save_customer_automation<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        data: &'a Value,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, ()>;
    /// Stores the rule metadata of a customer or order only at revision `rev`; the number of rows changed.
    fn save_automation_at<'a>(
        &'a self,
        a: &'a State,
        entity: RuleEntity,
        record: (&'a str, &'a str),
        data: &'a Value,
        rev: i64,
    ) -> PortFuture<'a, u64>;
    /// The order history of customer `customer`.
    fn customer_orders<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        customer: Option<&'a str>,
    ) -> PortFuture<'a, CustomerOrders>;
    /// The activity a flow step with idempotency key `key` already journaled on order `id`.
    fn flow_activity<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        record: (&'a str, &'a str),
        key: &'a str,
    ) -> PortFuture<'a, Option<Value>>;
    /// The document of order `id`, locked for the change.
    fn lock_order<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Value>;
    /// Stores the document of order `id`.
    fn save_order<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        data: &'a Value,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, ()>;
    /// Journals the effect of a flow step on order `id`.
    fn add_flow_activity<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        record: (&'a str, &'a str),
        actor: Option<&'a str>,
        data: &'a Value,
    ) -> PortFuture<'a, ()>;
    /// The kinds of the receipts issued for order `id`.
    fn receipt_kinds<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Vec<String>>;
    /// How many orders, carts and customers belong to sales channel `channel`.
    fn channel_usage<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        channel: &'a str,
    ) -> PortFuture<'a, [(&'static str, i64); 3]>;
    /// The shop's active app packages by id.
    fn active_apps<'a>(&'a self, a: &'a State, tenant: &'a str) -> PortFuture<'a, Vec<ActiveApp>>;
    /// The states of the work tasks among `ids`, in no particular order.
    fn task_states<'a>(
        &'a self,
        a: &'a State,
        tenant: &'a str,
        ids: &'a [String],
    ) -> PortFuture<'a, Vec<TaskState>>;
}

static RECORDS: OnceLock<Box<dyn Records>> = OnceLock::new();

/// Installs the server's records port at startup; a second installation keeps the first.
pub fn install(records: Box<dyn Records>) {
    let _ = RECORDS.set(records);
}

/// The installed records port; refused while the server has not installed it.
pub fn records() -> veyndo_core::Result<&'static dyn Records> {
    RECORDS.get().map(Box::as_ref).ok_or(veyndo_core::Error(
        veyndo_core::StatusCode::INTERNAL_SERVER_ERROR,
        "Marketing records port is not installed".into(),
    ))
}
