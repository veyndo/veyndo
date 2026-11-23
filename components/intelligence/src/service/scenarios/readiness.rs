//! Read-only readiness of a private sandbox as a shop twin: catalogue, stock, simulated checkout and saved customer tests.
use super::{attack_store, catalogue};
use crate::ports::AgentHost;
use serde_json::Value;
use serde_json::json;
use sqlx::Row;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::shop_scope::channel as channel_id;
use veyndo_core::shop_scope::header;
pub async fn read<H: AgentHost>(host: &H, h: &RequestContext, t: &str) -> Result<Value> {
    let a = host.models().core;
    let settings = catalogue::settings(a, h, t).await?;
    let currency = settings.currencies.selected("")?;
    let channel = channel_id(h);
    // Purchasable units are products without variants and the variants themselves.
    let row = sqlx::query("SELECT count(*) AS products, count(*) FILTER (WHERE active AND stock>0) AS stocked, count(*) FILTER (WHERE active AND stock<=0) AS empty, count(*) FILTER (WHERE NOT active) AS inactive FROM products p WHERE tenant=$1 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=p.tenant AND c.parent_id=p.id)")
        .bind(t).fetch_one(&a.db).await?;
    let (products, stocked): (i64, i64) = (row.get("products"), row.get("stocked"));
    let name: String = sqlx::query_scalar(
        "SELECT name FROM shop_environments WHERE tenant=$1 AND preview_owner IS NULL",
    )
    .bind(t)
    .fetch_one(&a.db)
    .await?;
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
    let tests: i64 = sqlx::query_scalar("SELECT count(DISTINCT run_request->'scenario'->>'group') FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'scenario' AND (run_request->>'agentId') IS NOT DISTINCT FROM $3")
        .bind(t).bind(actor).bind(&h.principal.agent_key).fetch_one(&a.db).await?;
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
        "catalogue":{"products":products,"withStock":stocked,"withoutStock":row.get::<i64,_>("empty"),"inactive":row.get::<i64,_>("inactive"),"visibleSample":visible},
        "checkout":{"country":country,"simulatedPayment":payment,"shipping":shipping},
        "tests":{"customerTests":tests,"attack":if attack.is_null() {Value::Null} else {json!({"summary":attack["summary"],"savedAt":attack["savedAt"],"configurationReview":attack["configurationReview"]})}},"ready":blockers.is_empty(),"blockers":blockers,"synthetic":true}),
    )
}
