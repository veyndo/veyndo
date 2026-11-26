//! The server's implementation of the shipping method references port: orders, carts, overrides, rules, flows and purchasing units counted by their owners' stores.
use crate::commerce::Guards;
use sqlx::PgConnection;
use veyndo_core::PortFuture;
use veyndo_shipping::ports::{MethodReference, MethodReferences};

impl MethodReferences for Guards {
    fn orders<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_orders::store::references::method_orders(
            tx,
            m.tenant,
            m.snapshot,
            m.id,
            m.channel,
            m.selection,
        ))
    }
    fn carts<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_checkout::store::references::method_carts(
            tx,
            m.tenant,
            m.selection,
            m.id,
            m.channel,
            m.legacy,
        ))
    }
    fn overrides<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_shop::store::references::method_overrides(
            tx, m.tenant, m.area, m.id,
        ))
    }
    fn rules<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_marketing::store::references::mentions(
            tx, m.tenant, m.id,
        ))
    }
    fn units<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        m: &'a MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(veyndo_b2b::store::references::method_units(
            tx, m.tenant, m.allowed, m.id,
        ))
    }
}
