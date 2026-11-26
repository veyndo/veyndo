//! Capture timing is versioned shop and channel configuration; every attempt freezes the provider intent admitted here.
use crate::domain::capture_intent::intent;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad};
use veyndo_shop::Settings;

/// Checks every configured capture mode against the payment providers installed in the shop.
pub async fn validate(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
    settings: &Settings,
) -> Result<()> {
    let rows: Vec<Value> = sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND manifest ? 'paymentProvider'",
        tenant
    )
    .fetch_all(&mut **tx)
    .await?;
    let manifests: Vec<Manifest> = rows
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| bad("Invalid installed payment provider"))?;
    for payment in settings
        .payments
        .iter()
        .filter(|p| p.capture_mode.is_some())
    {
        let method = manifests
            .iter()
            .find(|m| Some(&m.id) == payment.provider.as_ref())
            .and_then(|m| m.payment_provider.as_ref())
            .and_then(|p| {
                p.methods
                    .iter()
                    .find(|m| Some(&m.id) == payment.provider_method.as_ref())
            })
            .ok_or(bad(
                "Configured capture mode requires an installed payment provider",
            ))?;
        intent(payment.capture_mode.as_deref(), method)?;
    }
    Ok(())
}
