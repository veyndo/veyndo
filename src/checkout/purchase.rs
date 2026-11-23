//! The server side of the purchase transaction: app sandboxes and hooks, the cashier, the built-in payment provider and the agent run fence.
use crate::{
    App, RequestContext, Result, Sandbox, StoredCart, Value, agent_runs, apps, bad, commerce,
    conflict, payments, pos, sandbox_cache,
};
use std::{collections::HashMap, sync::Arc};
use veyndo_checkout::ports::{LockedQuote, Purchase};
use veyndo_core::PortFuture;

/// The modules a purchase prepared before its locks.
pub(crate) struct Modules {
    configurations: HashMap<String, Arc<Sandbox>>,
    hooks: apps::PreparedCheckoutHooks,
    policy: Option<Arc<Sandbox>>,
}

/// The server's half of one purchase.
pub(super) struct Server<'a>(pub(super) &'a App);

type Tx<'c> = sqlx::Transaction<'c, sqlx::Postgres>;

impl Purchase for Server<'_> {
    type Prepared = Modules;
    fn prepare<'a>(&'a self, cart: &'a StoredCart, business: bool) -> PortFuture<'a, Modules> {
        Box::pin(async move {
            let configurations = apps::prepare_configurations(self.0, cart).await?;
            let hooks = apps::prepare_hooks(self.0, &cart.tenant).await?;
            let policy = if business {
                Some(sandbox_cache::prepare(self.0, &cart.tenant).await?)
            } else {
                None
            };
            Ok(Modules {
                configurations,
                hooks,
                policy,
            })
        })
    }
    fn methods_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        cart: &'a StoredCart,
        currency: &'a str,
        settings: commerce::Settings,
    ) -> PortFuture<'a, commerce::Settings> {
        Box::pin(payments::currency_methods_conn(
            tx,
            &cart.tenant,
            &cart.data.sales_channel,
            currency,
            settings,
        ))
    }
    fn register_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        h: &'a RequestContext,
        cart: &'a StoredCart,
    ) -> PortFuture<'a, ()> {
        Box::pin(pos::check_locked(tx, h, cart))
    }
    fn configurations_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        cart: &'a StoredCart,
        prepared: &'a Modules,
    ) -> PortFuture<'a, ()> {
        Box::pin(apps::validate_configurations(
            tx,
            cart,
            &prepared.configurations,
        ))
    }
    fn adjust_locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        locked: LockedQuote<'a>,
        quote: Value,
        prepared: &'a Modules,
    ) -> PortFuture<'a, Value> {
        Box::pin(async move {
            let q = apps::apply_hooks(
                tx,
                locked.cart,
                locked.products,
                quote,
                locked.settings,
                locked.revision,
                &prepared.hooks,
            )
            .await?;
            pos::adjust_quote(
                locked.cart,
                q,
                locked.products,
                locked.settings,
                locked.revision,
            )
        })
    }
    fn approve(&self, prepared: &Modules, digest: &str, minor: i64) -> Result<()> {
        let sandbox = prepared
            .policy
            .as_ref()
            .filter(|s| s.digest() == digest)
            .ok_or(conflict(
                "Business policy changed during checkout; retry your reviewed purchase",
            ))?;
        if !sandbox.approve(minor, 100_000).map_err(bad)? {
            return Err(conflict("Company purchase rejected by tenant extension"));
        }
        Ok(())
    }
    fn settlement(
        &self,
        h: &RequestContext,
        cart: &StoredCart,
        quote: &Value,
        settings: &commerce::Settings,
        minor: i64,
    ) -> Result<()> {
        pos::validate_settlement(h, cart, quote, settings, minor)
    }
    fn provider_ready(&self, tenant: &str, payment_method: &str) -> Result<()> {
        payments::account(tenant)?;
        payments::base()?;
        if (payment_method == "paypal-live") != (payments::environment() == "live") {
            return Err(bad("Payment method environment mismatch"));
        }
        Ok(())
    }
    fn bind_order<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        tenant: &'a str,
        order: &'a mut Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(apps::native_lifecycle::bind_order(tx, tenant, order))
    }
    fn settle<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        h: &'a RequestContext,
        cart: &'a StoredCart,
        order: &'a mut Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(pos::settle(tx, h, cart, order))
    }
    fn prepare_payment<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut Tx<'c>,
        cart: &'a StoredCart,
        order: &'a mut Value,
        minor: i64,
        return_origin: Option<&'a str>,
    ) -> PortFuture<'a, ()> {
        Box::pin(payments::prepare(tx, cart, order, minor, return_origin))
    }
    fn fence<'a, 'c: 'a>(&'a self, tx: &'a mut Tx<'c>) -> PortFuture<'a, ()> {
        Box::pin(agent_runs::checkpoint::fence_commit(self.0, tx))
    }
}
