//! The order list query is owned by the orders component; admitted app filters join it here.
use super::*;
pub(super) async fn rows(
    a: &App,
    t: &str,
    c: &Criteria,
    n: i64,
    filters: &[apps::extension_filter::Filter],
) -> Result<Vec<sqlx::postgres::PgRow>> {
    veyndo_orders::store::order_list::rows(a, t, c, n, &|q| {
        apps::extension_filter::predicate(q, apps::core_extensions::Object::Order, t, filters)
    })
    .await
}
