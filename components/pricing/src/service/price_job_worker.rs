//! Bounded restart-safe fixed-price materialization with frozen FX, product locks and atomic checkpoints across replicas.
use crate::catalog::Product;
use crate::ports::Pricing;
use crate::service::conversion::{amount, convert_from};
use crate::store::price_jobs;
use crate::types::currency::{Config, Definition};
use crate::types::price_job::{JobData, QueuedJob};
use crate::types::product_currency::ProductCurrencies;
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::document::view;
use veyndo_core::{Result, bad};

/// The fixed prices of one product in the job's currency, converted with the rates frozen in the job.
pub fn fixed_prices(
    p: &Product,
    cfg: &Config,
    d: &Definition,
    generated_at: &str,
) -> Result<Value> {
    let currencies = ProductCurrencies::of(&p.extra);
    let source = currencies
        .price_currency
        .as_deref()
        .unwrap_or(&cfg.pricing_currency);
    let exact = |value: f64| -> Result<String> {
        Ok(amount(convert_from(value, source, cfg, d)?, &d.code, d.scale)?.decimal())
    };
    let mut prices = json!({"price":exact(p.price)?,"generatedAt":generated_at});
    for (key, value) in [
        ("listPrice", p.list_price),
        ("regulationPrice", p.regulation_price),
    ] {
        if let Some(value) = value {
            prices[key] = json!(exact(value)?);
        }
    }
    Ok(prices)
}

async fn step(p: &Pricing, tx: &mut PgConnection, job: &QueuedJob) -> Result<()> {
    let data: JobData = view(&job.data);
    p.catalogue
        .attribute(tx, data.actor.as_deref(), "currency-price-job")
        .await?;
    let cfg: Config =
        serde_json::from_value(data.config.clone()).map_err(|_| bad("Invalid price job"))?;
    let d = cfg
        .definitions
        .iter()
        .find(|d| Some(d.code.as_str()) == data.currency.as_deref())
        .ok_or(bad("Missing price job currency"))?;
    let products = p
        .catalogue
        .lock_products(tx, &job.tenant, &job.cursor, &job.ceiling, &job.snapshot)
        .await?;
    let mut cursor = job.cursor.clone();
    for product in &products {
        cursor = product.id.clone();
        if data.overwrite != Some(true) && ProductCurrencies::of(&product.extra).has_fixed(&d.code)
        {
            continue;
        }
        let prices = fixed_prices(product, &cfg, d, &chrono::Utc::now().to_rfc3339())?;
        p.catalogue
            .store_fixed_prices(tx, &job.tenant, &product.id, &d.code, prices)
            .await?;
    }
    let state = if products.len() < 100 {
        "completed"
    } else {
        "queued"
    };
    price_jobs::advance(tx, job, &cursor, products.len() as i64, state).await
}

/// Processes one batch of the oldest queued job; `false` when nothing is queued.
/// A rejected batch marks the job failed and keeps nothing of it; a server error leaves the job queued.
pub async fn batch(p: &Pricing) -> Result<bool> {
    let mut tx = p.core.db.begin().await?;
    let Some(job) = price_jobs::claim(&mut tx).await? else {
        return Ok(false);
    };
    price_jobs::begin_batch(&mut tx).await?;
    if let Err(error) = step(p, &mut tx, &job).await {
        if error.0.is_server_error() {
            return Err(error);
        }
        price_jobs::discard_batch(&mut tx).await?;
        price_jobs::fail(&mut tx, &job, &error.1).await?;
    }
    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::fixed_prices;
    use crate::catalog::Product;
    use crate::types::currency::{Config, Definition};
    use serde_json::json;

    fn config() -> Config {
        let mut config = Config::default();
        for (code, scale, rate) in [("USD", 2, "1.25"), ("KWD", 3, "0.33333333")] {
            config.enabled.push(code.into());
            config.definitions.push(Definition {
                code: code.into(),
                scale,
                rate: rate.into(),
                strategy: "fixed".into(),
            });
        }
        config
    }

    fn product(price: f64, list_price: Option<f64>, extra: serde_json::Value) -> Product {
        serde_json::from_value(json!({
            "extra":extra,"id":"p1","name":"Lamp","category":"","description":"","price":price,
            "tax_rate":19.0,"stock":1,"revision":1,"list_price":list_price,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }

    #[test]
    fn prices_are_exact_decimal_strings_at_the_currency_scale() {
        let config = config();
        let product = product(9.9, Some(12.0), json!({}));
        let usd = fixed_prices(&product, &config, &config.definitions[1], "now").unwrap();
        assert_eq!(
            usd,
            json!({"price":"12.38","listPrice":"15.00","generatedAt":"now"})
        );
        let kwd = fixed_prices(&product, &config, &config.definitions[2], "now").unwrap();
        assert_eq!(kwd["price"], "3.300");
        assert!(kwd.get("regulationPrice").is_none());
    }

    #[test]
    fn a_product_priced_in_another_currency_converts_from_that_currency() {
        let config = config();
        let product = product(12.5, None, json!({"priceCurrency":"USD"}));
        let eur = fixed_prices(&product, &config, &config.definitions[0], "now").unwrap();
        assert_eq!(eur["price"], "10.00");
    }
}
