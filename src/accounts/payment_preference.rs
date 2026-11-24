//! Optional last-used payment preference is written after purchase commit, without holding cart/unit/product locks.
use crate::{App, StoredCart};
pub(crate) async fn remember_payment(a: &App, c: &StoredCart, method: &str) {
    let Some(customer) = &c.data.customer_id else {
        return;
    };
    // A convenience default is not part of the financial order. Concurrent preflight
    // readers hold customer SHARE while waiting for the unit: updating it inside
    // checkout created a customer/unit lock inversion. Never fail a committed
    // purchase because this non-financial preference could not be remembered.
    if veyndo_customers::store::customers::remember_payment(&a.core, &c.tenant, customer, method)
        .await
        .is_err()
    {
        eprintln!("Customer payment preference update failed after committed purchase");
    }
}
