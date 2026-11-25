//! The purchase records of components that build on checkout: the business policy digest (apps), the experiment policy outcome (intelligence) and the funding buyer of a payment (customers for payments), on the purchase connection.
use crate::checkout::purchase::Server;
use veyndo_checkout::ports::PurchaseRecords;
use veyndo_core::PortFuture;

impl PurchaseRecords for Server<'_> {
    fn policy_digest<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, String> {
        Box::pin(veyndo_app_platform::store::policy_lock::locked_digest(
            conn, tenant,
        ))
    }
    fn count_purchase<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        variant: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_intelligence::store::policy_outcomes::count_purchase(
            conn, tenant, variant,
        ))
    }
}

impl veyndo_payments::ports::Buyers for Server<'_> {
    fn session_customer<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        digest: &'a str,
    ) -> PortFuture<'a, Option<String>> {
        Box::pin(
            veyndo_customers::store::purchase_customers::session_customer(conn, tenant, digest),
        )
    }
}
