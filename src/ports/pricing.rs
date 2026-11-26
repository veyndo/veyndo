//! The server's implementation of the pricing references port: products, open carts and purchasing units read through their owners' stores.
use crate::commerce::Guards;
use sqlx::PgConnection;
use veyndo_core::PortFuture;
use veyndo_pricing::ports::References;

impl References for Guards {
    fn product_ceiling<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, String> {
        Box::pin(veyndo_catalog::store::references::highest_id(tx, tenant))
    }
    fn products_use<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        codes: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(veyndo_catalog::store::references::currency_products(
            tx, tenant, codes,
        ))
    }
    fn carts_use<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        codes: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(veyndo_checkout::store::references::currency_carts(
            tx, tenant, codes,
        ))
    }
    fn units_use<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        codes: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(veyndo_b2b::store::references::currency_units(
            tx, tenant, codes,
        ))
    }
}
