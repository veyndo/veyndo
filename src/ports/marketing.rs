//! The server side of the marketing records port: carts of checkout, customers, orders, active app packages and work tasks, each read or changed by its owner's store on marketing's connection.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_app_platform::store::active_packages;
use veyndo_checkout::store::automation_carts;
use veyndo_checkout::store::carts::{self, CartRow};
use veyndo_core::{PortFuture, State};
use veyndo_customers::store::rule_customers;
use veyndo_intelligence::store::flow_tasks;
use veyndo_marketing::ports::{CustomerOrders, Records, RuleCustomer, RuleEntity, TaskState};
use veyndo_marketing::store::catalog::ActiveApp;
use veyndo_orders::store::rule_orders;
use veyndo_shop::types::cart::StoredCart;

/// The owners' stores behind the marketing records port.
pub(crate) struct AutomationRecords;

impl Records for AutomationRecords {
    fn lock_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>> {
        Box::pin(async move {
            carts::lock_by_token(conn, tenant, token)
                .await?
                .map(CartRow::into_cart)
                .transpose()
        })
    }
    fn save_cart_next_revision<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        data: &'a Value,
        (tenant, id): (&'a str, &'a str),
    ) -> PortFuture<'a, ()> {
        Box::pin(automation_carts::save_next_revision(conn, data, id, tenant))
    }
    fn order_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        order: &'a str,
    ) -> PortFuture<'a, Option<(StoredCart, Value)>> {
        Box::pin(async move {
            // An order without a cart, or whose cart is gone, has no cart to evaluate rules against.
            let Some(order) = rule_orders::with_cart(conn, tenant, order).await? else {
                return Ok(None);
            };
            let Some(cart_id) = order.cart_id else {
                return Ok(None);
            };
            let Some(cart) = automation_carts::by_id(conn, &cart_id).await? else {
                return Ok(None);
            };
            Ok(Some((cart.into_cart()?, order.data)))
        })
    }
    fn sign_out_carts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        seed: &'a str,
        tenant: &'a str,
        customer: Option<&'a str>,
    ) -> PortFuture<'a, ()> {
        Box::pin(automation_carts::sign_out(conn, seed, tenant, customer))
    }
    fn rule_customer<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Option<RuleCustomer>> {
        Box::pin(async move {
            Ok(rule_customers::rule_facts(conn, tenant, id)
                .await?
                .map(|r| RuleCustomer {
                    active: r.active,
                    email: r.email,
                    group_name: r.group_name,
                    customer_number: r.customer_number,
                    profile: r.profile,
                    automation: r.automation,
                    company: r.company,
                    age: r.age,
                    first_days: r.first_days,
                    last_days: r.last_days,
                }))
        })
    }
    fn lock_customer_group<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(rule_customers::lock_group(conn, tenant, id))
    }
    fn lock_customer_automation<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Option<Value>> {
        Box::pin(rule_customers::lock_automation(conn, tenant, id))
    }
    fn set_customer_group_and_status<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        change: (Option<&'a str>, Option<bool>),
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, u64> {
        Box::pin(rule_customers::set_group_and_status(
            conn, change, tenant, id,
        ))
    }
    fn end_customer_sessions<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, ()> {
        Box::pin(rule_customers::end_sessions(conn, tenant, id))
    }
    fn save_customer_automation<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        data: &'a Value,
        tenant: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, ()> {
        Box::pin(rule_customers::save_automation(conn, data, tenant, id))
    }
    fn save_automation_at<'a>(
        &'a self,
        a: &'a State,
        entity: RuleEntity,
        (tenant, id): (&'a str, &'a str),
        data: &'a Value,
        rev: i64,
    ) -> PortFuture<'a, u64> {
        Box::pin(async move {
            match entity {
                RuleEntity::Customer => {
                    rule_customers::save_automation_at(a, tenant, id, data, rev).await
                }
                RuleEntity::Order => {
                    rule_orders::save_automation_at(a, tenant, id, data, rev).await
                }
            }
        })
    }
    fn customer_orders<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        customer: Option<&'a str>,
    ) -> PortFuture<'a, CustomerOrders> {
        Box::pin(async move {
            let r = rule_orders::customer_orders(conn, tenant, customer).await?;
            Ok(CustomerOrders {
                count: r.count,
                amount: r.amount,
                days: r.days,
            })
        })
    }
    fn flow_activity<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        (tenant, id): (&'a str, &'a str),
        key: &'a str,
    ) -> PortFuture<'a, Option<Value>> {
        Box::pin(rule_orders::flow_activity(conn, tenant, id, key))
    }
    fn lock_order<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Value> {
        Box::pin(rule_orders::lock_data(conn, tenant, id))
    }
    fn save_order<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        data: &'a Value,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(rule_orders::save_data(conn, data, tenant, id))
    }
    fn add_flow_activity<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        (tenant, id): (&'a str, &'a str),
        actor: Option<&'a str>,
        data: &'a Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(rule_orders::add_flow_activity(
            conn, tenant, id, actor, data,
        ))
    }
    fn receipt_kinds<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(rule_orders::receipt_kinds(conn, tenant, id))
    }
    fn channel_usage<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        channel: &'a str,
    ) -> PortFuture<'a, [(&'static str, i64); 3]> {
        Box::pin(async move {
            let orders = rule_orders::channel_count(conn, tenant, channel).await?;
            let carts = automation_carts::channel_count(conn, tenant, channel).await?;
            let customers = rule_customers::channel_count(conn, tenant, channel).await?;
            Ok([
                ("orders", orders),
                ("carts", carts),
                ("customers", customers),
            ])
        })
    }
    fn active_apps<'a>(&'a self, a: &'a State, tenant: &'a str) -> PortFuture<'a, Vec<ActiveApp>> {
        Box::pin(async move {
            Ok(active_packages::active(a, tenant)
                .await?
                .into_iter()
                .map(|p| ActiveApp {
                    id: p.id,
                    manifest: p.manifest,
                })
                .collect())
        })
    }
    fn task_states<'a>(
        &'a self,
        a: &'a State,
        tenant: &'a str,
        ids: &'a [String],
    ) -> PortFuture<'a, Vec<TaskState>> {
        Box::pin(async move {
            Ok(flow_tasks::states(a, tenant, ids)
                .await?
                .into_iter()
                .map(|t| TaskState {
                    id: t.id,
                    applied: t.applied,
                    execution_started: t.execution_started,
                })
                .collect())
        })
    }
}
