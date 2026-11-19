//! Queues a run that writes fixed prices of one currency, with the exchange rates frozen at creation.
use crate::ports::Pricing;
use crate::store::price_jobs;
use serde_json::json;
use veyndo_core::{Error, Result, StatusCode, bad, conflict, uid};

pub struct NewJob<'a> {
    pub currency: Option<&'a str>,
    pub overwrite: bool,
    /// The settings revision the merchant saw; a different current one refuses the job.
    pub revision: Option<i64>,
    pub actor: &'a str,
}

pub async fn create(p: &Pricing, tenant: &str, job: NewJob<'_>) -> Result<String> {
    let mut tx = p.core.db.begin().await?;
    let (currencies, revision) = p.settings.currencies_for_update(&mut tx, tenant).await?;
    if price_jobs::queued(&mut tx, tenant).await? >= 3 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Maximum three pending currency jobs".into(),
        ));
    }
    if job.revision != Some(revision) {
        return Err(conflict("Settings changed"));
    }
    let code = job.currency.ok_or(bad("Currency required"))?;
    let definition = currencies
        .definitions
        .iter()
        .find(|d| d.code == code && d.strategy == "fixed")
        .ok_or(bad("Select a fixed-price currency"))?;
    // Freshness is checked even when this currency is only enabled on another channel.
    let mut enabled = currencies.clone();
    enabled.enabled.push(code.into());
    enabled.selected(code)?;
    let ceiling = price_jobs::product_ceiling(&mut tx, tenant).await?;
    let id = uid();
    let data = json!({"config":currencies,"currency":definition.code,"overwrite":job.overwrite,"actor":job.actor,"settingsRevision":revision});
    price_jobs::insert(&mut tx, tenant, &id, data, &ceiling).await?;
    tx.commit().await?;
    Ok(id)
}
