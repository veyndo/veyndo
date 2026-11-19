//! Merchant-owned guardrails in commerce settings; native currency amounts bind preview and execution.
use serde::{Deserialize, Serialize};
use veyndo_core::{Result, bad};
use veyndo_pricing::types::currency::Config as Currencies;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub corridors: Vec<Corridor>,
    #[serde(default)]
    pub autonomy: Autonomy,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Corridor {
    pub product_id: String,
    pub currency: String,
    pub minimum_minor: u64,
    pub maximum_minor: u64,
    #[serde(default)]
    pub minimum_margin_bps: u16,
    #[serde(default)]
    pub unit_cost_net_minor: Option<u64>,
    #[serde(default)]
    pub maximum_discount_bps: Option<u16>,
    #[serde(default)]
    pub price_locked: bool,
    #[serde(default)]
    pub require_available: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Autonomy {
    pub enabled: bool,
    pub max_change_bps: u16,
    pub products_per_day: u16,
    /// Optional ceiling for today's price reductions times current stock, in minor units of the pricing currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_daily_exposure_minor: Option<u64>,
}
impl Default for Autonomy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_change_bps: 500,
            products_per_day: 20,
            max_daily_exposure_minor: None,
        }
    }
}
impl Config {
    /// Bounds of the guardrails; a corridor must name a configured currency.
    pub fn validate(&self, currencies: &Currencies) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        if self.corridors.len() > 1000
            || self.autonomy.max_change_bps > 500
            || !(1..=100).contains(&self.autonomy.products_per_day)
            || self
                .autonomy
                .max_daily_exposure_minor
                .is_some_and(|limit| limit > 1_000_000_000_000)
        {
            return Err(bad("AI guardrail limits exceeded"));
        }
        for r in &self.corridors {
            if !ids.insert(&r.product_id)
                || r.product_id.is_empty()
                || r.product_id.len() > 160
                || r.minimum_minor > r.maximum_minor
                || r.maximum_minor > 1_000_000_000_000
                || r.minimum_margin_bps >= 10000
                || r.minimum_margin_bps > 0 && r.unit_cost_net_minor.is_none()
                || r.maximum_discount_bps.is_some_and(|v| v > 10000)
                || !currencies.definitions.iter().any(|d| d.code == r.currency)
            {
                return Err(bad("Invalid AI product corridor or cost/margin basis"));
            }
        }
        Ok(())
    }
}
