//! The funding choice of a payment: the buyer's session, the saved instrument it reuses and the consent bound to the attempt. Columns checked at compile time.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;

/// The provider binding of a saved instrument.
pub struct SavedInstrument {
    pub reference: String,
    pub method: String,
    pub provider: String,
    pub account_ref: String,
    pub environment: String,
}

/// An active instrument of the customer, share-locked so it cannot be revoked while the purchase commits.
pub async fn share_instrument(
    tx: &mut PgConnection,
    tenant: &str,
    customer: &str,
    id: &str,
) -> Result<Option<SavedInstrument>> {
    Ok(sqlx::query_as!(
        SavedInstrument,
        "SELECT reference,method,provider,account_ref,environment FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND id=$3 AND state='active' FOR SHARE",
        tenant,
        customer,
        id
    )
    .fetch_optional(tx)
    .await?)
}

/// Binds the funding choice to the attempt; a new consent to save starts as `pending`, a reused instrument as `reused`.
pub async fn bind(
    tx: &mut PgConnection,
    tenant: &str,
    attempt: &str,
    funding: &Value,
) -> Result<()> {
    sqlx::query!("UPDATE payment_attempts SET provider_context=jsonb_set(jsonb_set(provider_context,'{funding}',$3),'{vaultStatus}',to_jsonb(CASE WHEN $3::jsonb->>'save'='true' THEN 'pending'::text ELSE 'reused'::text END)) WHERE tenant=$1 AND id=$2", tenant, attempt, funding).execute(tx).await?;
    Ok(())
}

/// The customer's instrument is still active.
pub async fn instrument_active<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
    customer: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM payment_instruments WHERE tenant=$1 AND id=$2 AND customer_id=$3 AND state='active') AS "active!""#,
        tenant,
        id,
        customer
    )
    .fetch_one(db)
    .await?)
}
