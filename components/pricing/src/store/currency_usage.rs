//! Where a shop still refers to a currency: queued price jobs here, product prices, open carts and purchasing units through the references port.
use crate::ports::References;
use sqlx::PgConnection;
use veyndo_core::Result;

pub async fn any_in_use(
    references: &dyn References,
    tx: &mut PgConnection,
    tenant: &str,
    codes: &[String],
) -> Result<bool> {
    Ok(references.products_use(&mut *tx, tenant, codes).await?
        || sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM currency_price_jobs WHERE tenant=$1 AND state='queued' AND data->>'currency'=ANY($2)) AS "in_use!""#, tenant, codes)
            .fetch_one(&mut *tx)
            .await?
        || references.carts_use(&mut *tx, tenant, codes).await?
        || references.units_use(tx, tenant, codes).await?)
}
