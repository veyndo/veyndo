//! Bounded ECB reference-rate retrieval with exact decimal parsing; a refresh never invents rates.
use crate::types::currency::{Config, rate_units};
use std::collections::HashMap;
use veyndo_core::{Result, bad};

pub async fn fetch(cfg: &Config) -> Result<Config> {
    static CACHE: std::sync::OnceLock<tokio::sync::Mutex<Option<(std::time::Instant, String)>>> =
        std::sync::OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| tokio::sync::Mutex::new(None))
        .lock()
        .await;
    if let Some((time, xml)) = &*cache
        && time.elapsed().as_secs() < 1800
    {
        return parse(cfg, xml);
    }
    let xml = download().await?;
    let result = parse(cfg, &xml)?;
    *cache = Some((std::time::Instant::now(), xml));
    Ok(result)
}
async fn download() -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| bad("FX client unavailable"))?;
    let response = client
        .get("https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml")
        .send()
        .await
        .map_err(|_| bad("ECB unavailable; previous rates retained"))?
        .error_for_status()
        .map_err(|_| bad("ECB rejected request; previous rates retained"))?;
    if response.content_length().is_some_and(|n| n > 100_000) {
        return Err(bad("ECB response too large"));
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| bad("ECB response interrupted"))?
    {
        if bytes.len() + chunk.len() > 100_000 {
            return Err(bad("ECB response too large"));
        }
        bytes.extend(chunk);
    }
    String::from_utf8(bytes).map_err(|_| bad("Invalid ECB response"))
}
pub fn parse(cfg: &Config, xml: &str) -> Result<Config> {
    let date = regex::Regex::new(r#"time=['"]([0-9]{4}-[0-9]{2}-[0-9]{2})['"]"#)
        .unwrap()
        .captures(xml)
        .map(|c| c[1].to_owned())
        .ok_or(bad("ECB rate date missing"))?;
    let day = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| bad("Invalid ECB date"))?;
    if !(0..=7).contains(&(chrono::Utc::now().date_naive() - day).num_days()) {
        return Err(bad("ECB response stale"));
    }
    let re = regex::Regex::new(r#"currency=['"]([A-Z]{3})['"]\s+rate=['"]([0-9.]+)['"]"#).unwrap();
    let mut rates = HashMap::from([("EUR".to_owned(), 100_000_000_i64)]);
    for c in re.captures_iter(xml) {
        if rates.insert(c[1].to_owned(), rate_units(&c[2])?).is_some() {
            return Err(bad("Duplicate ECB rate"));
        }
    }
    let base = *rates
        .get(&cfg.base_currency)
        .ok_or(bad("ECB does not publish the base currency"))? as i128;
    let mut next = cfg.clone();
    for d in &mut next.definitions {
        let target = *rates.get(&d.code).ok_or(bad(format!(
            "ECB does not publish {}; retain manual rates or remove it from this ECB configuration",
            d.code
        )))? as i128;
        let units = (target * 100_000_000 + base / 2) / base;
        d.rate = format!("{}.{:08}", units / 100_000_000, units % 100_000_000);
    }
    next.rate_source = "ecb".into();
    next.rate_date = Some(date);
    next.validate()?;
    Ok(next)
}
