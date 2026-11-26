//! Read-only readiness of a private sandbox as a shop twin: catalogue, stock, simulated checkout and saved customer tests.
use super::AttackSummary;
use super::{attack_store, catalogue};
use crate::ports::AgentHost;
use crate::store::scenario_reports as store;
use serde_json::Value;
use serde_json::json;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::view;
use veyndo_core::shop_scope::channel as channel_id;
use veyndo_core::shop_scope::header;
pub async fn read<H: AgentHost>(host: &H, h: &RequestContext, t: &str) -> Result<Value> {
    let a = host.models().core;
    let settings = catalogue::settings(a, h, t).await?;
    let currency = settings.currencies.selected("")?;
    let channel = channel_id(h);
    // Purchasable units are products without variants and the variants themselves.
    let row = store::catalogue_counts(&a.db, t).await?;
    let (products, stocked) = (row.products, row.stocked);
    let name = store::sandbox_name(&a.db, t).await?;
    let visible = match settings
        .locales
        .iter()
        .find(|l| *l == &settings.main_locale)
        .or_else(|| settings.locales.first())
    {
        Some(locale) => catalogue::sample(host, t, channel, locale, &currency.code, currency.scale)
            .await?
            .as_array()
            .map_or(0, Vec::len),
        None => 0,
    };
    let country = settings.countries.first().cloned();
    let payment = country.as_deref().is_some_and(|country| {
        settings.payments.iter().any(|p| {
            p.active && p.mode == "simulated" && !p.business_only && p.available_in(country)
        })
    });
    let shipping = country.as_deref().is_some_and(|country| {
        settings
            .shipping
            .iter()
            .any(|s| s.active && s.countries.iter().any(|c| c == country))
    });
    let actor = header(h, "x-rac-user").ok_or(bad("Scenario actor required"))?;
    let tests = store::scenario_groups(&a.db, t, actor, h.principal.agent_key.as_deref()).await?;
    let attack = attack_store::latest(host, h, t).await?;
    let blockers: Vec<&str> = [
        (products == 0, "no_products"),
        (products > 0 && visible == 0, "channel_empty"),
        (products > 0 && stocked == 0, "no_stock"),
        (country.is_none(), "no_country"),
        (country.is_some() && !payment, "no_payment"),
        (country.is_some() && !shipping, "no_shipping"),
    ]
    .into_iter()
    .filter_map(|(blocked, code)| blocked.then_some(code))
    .collect();
    Ok(
        json!({"sandbox":{"id":t,"name":name},"channel":channel,"currency":currency.code,
        "catalogue":{"products":products,"withStock":stocked,"withoutStock":row.empty,"inactive":row.inactive,"visibleSample":visible},
        "checkout":{"country":country,"simulatedPayment":payment,"shipping":shipping},
        "tests":{"customerTests":tests,"attack":if attack.is_null() {Value::Null} else {let attack=view::<AttackSummary>(&attack);json!({"summary":attack.summary,"savedAt":attack.saved_at,"configurationReview":attack.configuration_review})}},"ready":blockers.is_empty(),"blockers":blockers,"synthetic":true}),
    )
}
