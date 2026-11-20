//! The customer a merchant selected for a cart, revalidated under a shared lock so profile edits cannot race the purchase.
use sqlx::{PgConnection, Row};
use veyndo_core::{Result, conflict};
use veyndo_shop::types::cart::StoredCart;

/// Revalidate the selected account under a shared lock inside checkout; profile edits cannot race settlement.
pub async fn check(conn: &mut PgConnection, c: &StoredCart) -> Result<()> {
    if let Some(id) = &c.data.customer_id {
        let row=sqlx::query("SELECT email,group_name,company FROM customers WHERE tenant=$1 AND id=$2 AND active FOR SHARE").bind(&c.tenant).bind(id).fetch_optional(conn).await?.ok_or(conflict("Selected customer is no longer active; select again"))?;
        if Some(row.get::<String, _>("email")) != c.data.email
            || row.get::<String, _>("group_name") != c.data.group
            || row.get::<Option<String>, _>("company") != c.data.company
        {
            return Err(conflict(
                "Customer changed; select again before completing sale",
            ));
        }
    }
    Ok(())
}
