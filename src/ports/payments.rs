//! The ledger records of components that build on payments: the cart order snapshot and the experiment reward (checkout) with its policy outcome (intelligence) in the ledger transaction, the payment history of an order (studio) on the reader snapshot, and the shop's sales channels (marketing).
use crate::payments::host::PaymentHost;
use serde_json::Value;
use veyndo_core::PortFuture;
use veyndo_payments::ports::LedgerRecords;

impl LedgerRecords for PaymentHost {
    fn mirror_cart_order<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        cart: &'a str,
        order: &'a Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_checkout::store::order_snapshot::mirror(
            conn, tenant, cart, order,
        ))
    }
    fn reward_capture<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        cart: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            if let Some(variant) =
                veyndo_checkout::store::purchase::reward_cart_exposure(&mut *conn, tenant, cart)
                    .await?
            {
                veyndo_intelligence::store::policy_outcomes::count_purchase(conn, tenant, &variant)
                    .await?;
            }
            Ok(())
        })
    }
}

impl veyndo_payments::ports::MerchantRecords for crate::payments::host::Gateway<'_> {
    fn payment_history<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        order: &'a str,
    ) -> PortFuture<'a, Vec<veyndo_payments::ports::PaymentChange>> {
        Box::pin(async move {
            Ok(
                veyndo_studio::store::payment_history::changes(conn, tenant, order)
                    .await?
                    .into_iter()
                    .map(|r| veyndo_payments::ports::PaymentChange {
                        id: r.id,
                        created_at: r.created_at,
                        state: r.state,
                        captured: r.captured,
                        refunded: r.refunded,
                    })
                    .collect(),
            )
        })
    }
    fn channel_exists<'a>(&'a self, tenant: &'a str, channel: &'a str) -> PortFuture<'a, bool> {
        Box::pin(veyndo_marketing::store::channel_ids::exists(
            &self.0.db, tenant, channel,
        ))
    }
    fn channel_ids<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Vec<String>> {
        Box::pin(veyndo_marketing::store::channel_ids::ids(
            &self.0.db, tenant,
        ))
    }
}
