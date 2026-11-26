//! The customer a merchant selected for a cart, revalidated under a shared lock so profile edits cannot race the purchase.
use sqlx::PgConnection;
use veyndo_core::{Result, conflict};
use veyndo_customers::store::purchase_customers::share_selected;
use veyndo_shop::types::cart::StoredCart;

/// Revalidate the selected account under a shared lock inside checkout; profile edits cannot race settlement.
pub async fn check(conn: &mut PgConnection, c: &StoredCart) -> Result<()> {
    if let Some(id) = &c.data.customer_id {
        let row = share_selected(conn, &c.tenant, id).await?.ok_or(conflict(
            "Selected customer is no longer active; select again",
        ))?;
        if Some(row.email) != c.data.email
            || row.group_name != c.data.group
            || row.company != c.data.company
        {
            return Err(conflict(
                "Customer changed; select again before completing sale",
            ));
        }
    }
    Ok(())
}
