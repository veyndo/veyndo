//! Recorded unit demand of products over a trailing window: the baseline a proposal simulation projects from, aggregated without any customer data.
use veyndo_core::Result;

/// Units of one product sold in placed, unpaid-or-paid, not cancelled orders of the window.
pub struct Demand {
    pub product_id: String,
    pub units: i64,
    pub orders: i64,
}

/// Units and orders per given product over the last `days` days; products without sales are absent.
pub async fn trailing(
    c: &mut sqlx::PgConnection,
    t: &str,
    ids: &[String],
    days: i32,
) -> Result<Vec<Demand>> {
    Ok(
        sqlx::query_file_as!(Demand, "src/store/product_demand.sql", t, ids, days)
            .fetch_all(&mut *c)
            .await?,
    )
}
