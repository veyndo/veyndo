//! The server's implementation of the shop dependants port: customers, orders, apps, rules and carts read by their owners' stores.
use crate::foundation::App;
use serde_json::Value;
use sqlx::PgConnection;
use std::collections::HashMap;
use std::sync::Arc;
use veyndo_core::PortFuture;
use veyndo_shop::ports::{Activity, Dependants};
use veyndo_shop::store::operator_metrics::{Amounts, Day};

struct Server(veyndo_core::State);

impl Dependants for Server {
    fn customers_outside<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        excluded: &'a [String],
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_customers::store::references::count_outside(
            conn, excluded,
        ))
    }
    fn orders_outside<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        days: i64,
        excluded: &'a [String],
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_orders::store::references::count_outside(
            conn, days, excluded,
        ))
    }
    fn amounts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        days: i64,
        tenant: Option<&'a str>,
        excluded: &'a [String],
    ) -> PortFuture<'a, Vec<Amounts>> {
        Box::pin(veyndo_orders::store::references::amounts(
            conn, days, tenant, excluded,
        ))
    }
    fn activity<'a>(
        &'a self,
        tenants: &'a [String],
        days: i64,
    ) -> PortFuture<'a, HashMap<String, Activity>> {
        Box::pin(async move {
            let mut held: HashMap<String, Activity> = HashMap::new();
            for (tenant, count) in
                veyndo_customers::store::references::count_by_shop(&self.0, tenants).await?
            {
                held.entry(tenant).or_default().customers = count;
            }
            for (tenant, count) in
                veyndo_orders::store::references::count_by_shop(&self.0, tenants, days).await?
            {
                held.entry(tenant).or_default().orders = count;
            }
            for (tenant, count) in
                veyndo_app_platform::store::shop_reads::active_by_shop(&self.0, tenants).await?
            {
                held.entry(tenant).or_default().apps = count;
            }
            for (tenant, count) in
                veyndo_app_platform::store::shop_reads::sources_by_shop(&self.0, tenants).await?
            {
                held.entry(tenant).or_default().sources = count;
            }
            Ok(held)
        })
    }
    fn timeline<'a>(&'a self, tenant: &'a str, days: i64) -> PortFuture<'a, Vec<Day>> {
        Box::pin(veyndo_orders::store::references::timeline(
            &self.0, tenant, days,
        ))
    }
    fn recent_orders<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Vec<Value>> {
        Box::pin(veyndo_orders::store::references::recent(&self.0, tenant))
    }
    fn receipt_identity<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Option<Value>> {
        Box::pin(veyndo_orders::store::references::receipt_identity(
            &self.0, tenant,
        ))
    }
    fn lock_cart<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_checkout::store::references::lock(tx, tenant, id))
    }
    fn customers_outside_groups<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        groups: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(veyndo_customers::store::references::outside_groups(
            tx, tenant, groups,
        ))
    }
    fn marketing_documents<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Vec<Value>> {
        Box::pin(veyndo_marketing::store::references::documents(tx, tenant))
    }
}

/// Registers the dependants port with the process state.
pub(super) fn register(a: &App) {
    veyndo_shop::ports::register_dependants(Arc::new(Server(a.core.clone())));
}
