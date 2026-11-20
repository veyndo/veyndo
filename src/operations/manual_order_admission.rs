//! Private admin-cart admission and locked final checkout guards are owned by the checkout component.
use super::*;
pub(crate) use veyndo_checkout::domain::admin_order::{
    admit_payment as admit_admin_payment, order as admin_order, purchase as admin_purchase,
};
pub(crate) use veyndo_checkout::store::admin_order::admit_products as admit_admin_products;
pub(crate) use veyndo_checkout::store::selected_customer::check as check_admin_customer;
/// Freeze channel policy in the existing checkout transaction before acquiring customer/cart locks.
pub(crate) async fn lock_admin_checkout(
    conn: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
) -> Result<()> {
    veyndo_checkout::store::admin_order::lock_checkout(conn, c, &marketing::validate_condition)
        .await
}
