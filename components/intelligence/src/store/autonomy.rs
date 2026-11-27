//! The autonomous price budget in storage: today's baselines, exposure rows, reservations and the reverting price write. Columns checked at compile time.
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};
use veyndo_catalog::store::exact::Exact;
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::decimal::Decimal;

/// Products the shop's autonomous runs changed today.
pub async fn used_today(e: impl PgExecutor<'_>, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM ai_price_budget WHERE tenant=$1 AND day=(now() AT TIME ZONE 'UTC')::date"#, tenant).fetch_one(e).await?)
}

/// The products the shop's autonomous runs changed today.
pub async fn products_today(e: impl PgExecutor<'_>, tenant: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT product_id FROM ai_price_budget WHERE tenant=$1 AND day=(now() AT TIME ZONE 'UTC')::date", tenant).fetch_all(e).await?)
}

/// Today's first change of a product: its price before the autonomous run.
pub struct Baseline<'a> {
    pub product: &'a str,
    pub minor: i64,
    pub currency: &'a str,
    pub task: &'a str,
    pub actor: &'a str,
}

/// Records today's baseline of a product (the first one stays) and returns the kept baseline.
pub async fn reserve(e: impl PgExecutor<'_>, tenant: &str, b: Baseline<'_>) -> Result<i64> {
    Ok(sqlx::query_scalar!("INSERT INTO ai_price_budget(tenant,day,product_id,baseline_minor,currency,task_id,actor) VALUES($1,(now() AT TIME ZONE 'UTC')::date,$2,$3,$4,$5,$6) ON CONFLICT(tenant,day,product_id) DO UPDATE SET task_id=EXCLUDED.task_id RETURNING baseline_minor",
        tenant, b.product, b.minor, b.currency, b.task, b.actor).fetch_one(e).await?)
}

/// The currency of a product's baseline today.
pub async fn baseline_currency(
    e: impl PgExecutor<'_>,
    tenant: &str,
    product: &str,
) -> Result<String> {
    Ok(sqlx::query_scalar!("SELECT currency FROM ai_price_budget WHERE tenant=$1 AND day=(now() AT TIME ZONE 'UTC')::date AND product_id=$2",
        tenant, product).fetch_one(e).await?)
}

/// A product changed today with its baseline and current state.
pub struct Exposure {
    pub product_id: String,
    pub baseline_minor: i64,
    pub currency: String,
    pub name: String,
    pub price: Exact,
    pub stock: i32,
}

/// Today's changed products of the shop.
pub async fn exposures(e: impl PgExecutor<'_>, tenant: &str) -> Result<Vec<Exposure>> {
    Ok(sqlx::query_as!(Exposure, "SELECT b.product_id,b.baseline_minor,b.currency,p.name,p.price_decimal AS \"price!: Exact\",p.stock FROM ai_price_budget b JOIN products p ON p.tenant=b.tenant AND p.id=b.product_id WHERE b.tenant=$1 AND b.day=(now() AT TIME ZONE 'UTC')::date ORDER BY b.product_id",
        tenant).fetch_all(e).await?)
}

/// A product changed today, as revert reads it.
pub struct Changed {
    pub product_id: String,
    pub baseline_minor: i64,
    pub currency: String,
    pub price: Exact,
    pub revision: i64,
    /// The changes of the run's proposal.
    pub changes: Option<Value>,
}

/// Today's changed products (or one), with their products row-locked.
pub async fn changed_locked(
    e: impl PgExecutor<'_>,
    tenant: &str,
    only: Option<&str>,
) -> Result<Vec<Changed>> {
    Ok(sqlx::query_as!(Changed, "SELECT b.product_id,b.baseline_minor,b.currency,p.price_decimal AS \"price!: Exact\",p.revision,k.proposal->'proposal'->'changes' AS changes FROM ai_price_budget b JOIN products p ON p.tenant=b.tenant AND p.id=b.product_id JOIN tasks k ON k.tenant=b.tenant AND k.id=b.task_id WHERE b.tenant=$1 AND b.day=(now() AT TIME ZONE 'UTC')::date AND ($2::text IS NULL OR b.product_id=$2) ORDER BY b.product_id FOR UPDATE OF p",
        tenant, only).fetch_all(e).await?)
}

/// Restores a product price at the expected revision; None when the product changed since.
pub async fn restore_price(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    price: Decimal,
    revision: i64,
) -> Result<Option<Product>> {
    // runtime SQL: the returned row goes to veyndo_catalog::store::products::product, which maps a PgRow.
    let row = sqlx::query("UPDATE products SET price_decimal=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4 RETURNING *")
        .bind(Exact(price)).bind(tenant).bind(id).bind(revision).fetch_optional(tx).await?;
    Ok(row.as_ref().map(veyndo_catalog::store::products::product))
}

/// Shops with autonomous price changes today (at most 500).
pub async fn shops_today(e: impl PgExecutor<'_>) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT DISTINCT tenant FROM ai_price_budget WHERE day=(now() AT TIME ZONE 'UTC')::date ORDER BY tenant LIMIT 500").fetch_all(e).await?)
}
