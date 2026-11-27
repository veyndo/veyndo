//! Exact rational minor-unit FX conversion on exact decimals; fixed prices are explicit decimal strings.
use crate::catalog::Product;
use crate::decimal::Decimal;
use crate::money::{Currency, Money};
use crate::types::currency::{Config, Definition, rate_units, scale};
use crate::types::product_currency::ProductCurrencies;
use serde_json::{Value, json};
use veyndo_core::{Result, bad};

/// A wire amount (a JSON number) as money in `code`, rounded half away from zero to `precision`.
/// The number is read as the shortest decimal that names it; negative and non-finite values are refused.
pub fn amount(value: f64, code: &str, precision: u8) -> Result<Money> {
    exact_amount(
        Decimal::from_wire(value).map_err(|_| bad("Invalid currency amount"))?,
        code,
        precision,
    )
}
/// An exact decimal amount as money in `code`, rounded half away from zero to `precision`.
pub fn exact_amount(value: Decimal, code: &str, precision: u8) -> Result<Money> {
    if value < Decimal::ZERO {
        return Err(bad("Invalid currency amount"));
    }
    Money::from_decimal(value, Currency::new(code, precision).map_err(bad)?).map_err(bad)
}

/// A price in the pricing currency, in the cart currency `d`.
pub fn convert(value: Decimal, cfg: &Config, d: &Definition) -> Result<Decimal> {
    convert_from(value, &cfg.pricing_currency, cfg, d)
}
/// A price in `code`, in the currency `d`: unchanged in the same currency, otherwise rounded to the
/// source's minor unit, converted with the exact rate ratio and rounded half up to the target's.
pub fn convert_from(value: Decimal, code: &str, cfg: &Config, d: &Definition) -> Result<Decimal> {
    if d.code == code {
        return Ok(value);
    }
    let source = exact_amount(
        value,
        code,
        scale(code).ok_or(bad("Invalid price source currency"))?,
    )?;
    let source_rate = cfg
        .definitions
        .iter()
        .find(|v| v.code == code)
        .ok_or(bad("Source price currency is not configured"))?;
    let numerator = (source.minor() as i128)
        .checked_mul(rate_units(&d.rate)? as i128)
        .and_then(|v| v.checked_mul(10_i128.pow(d.scale as u32)))
        .ok_or(bad("FX overflow"))?;
    let denominator =
        rate_units(&source_rate.rate)? as i128 * 10_i128.pow(source.currency().scale() as u32);
    let minor = i64::try_from((numerator + denominator / 2) / denominator)
        .map_err(|_| bad("FX overflow"))?;
    Decimal::new(minor.into(), d.scale.into()).map_err(bad)
}
/// A fixed per-currency price field as an exact decimal at the currency's scale.
fn fixed(text: &str, d: &Definition) -> Result<Decimal> {
    Ok(
        Money::parse(text, Currency::new(&d.code, d.scale).map_err(bad)?)
            .map_err(bad)?
            .amount()
            .decimal(),
    )
}
/// A stored binary64 amount (product columns, settings) as the exact decimal it names.
pub fn stored(value: f64) -> Result<Decimal> {
    Decimal::from_wire(value).map_err(|_| bad("Invalid stored amount"))
}
pub fn product_price(p: &Product, cfg: &Config, d: &Definition) -> Result<Decimal> {
    if d.strategy == "fixed"
        && let Some(s) = ProductCurrencies::of(&p.extra)
            .fixed(&d.code)
            .field("price")
    {
        return fixed(s, d);
    }
    convert_from(
        p.price,
        ProductCurrencies::of(&p.extra)
            .price_currency
            .as_deref()
            .unwrap_or(&cfg.pricing_currency),
        cfg,
        d,
    )
}
pub fn ancillary(
    p: &Product,
    field: &str,
    value: Option<Decimal>,
    cfg: &Config,
    d: &Definition,
) -> Result<Option<Decimal>> {
    if d.strategy == "fixed"
        && let Some(s) = ProductCurrencies::of(&p.extra).fixed(&d.code).field(field)
    {
        return Ok(Some(fixed(s, d)?));
    }
    value
        .map(|v| {
            convert_from(
                v,
                ProductCurrencies::of(&p.extra)
                    .price_currency
                    .as_deref()
                    .unwrap_or(&cfg.pricing_currency),
                cfg,
                d,
            )
        })
        .transpose()
}
pub fn validate_prices(v: &Value, cfg: &Config) -> Result<()> {
    if v.is_null() {
        return Ok(());
    }
    let map = v
        .as_object()
        .filter(|v| v.len() <= 40)
        .ok_or(bad("Currency prices must be an object"))?;
    for (code, prices) in map {
        let d = cfg
            .definitions
            .iter()
            .find(|d| &d.code == code)
            .ok_or(bad("Unknown product currency"))?;
        let values = prices.as_object().ok_or(bad("Invalid currency prices"))?;
        if values.len() > 4 || !values.contains_key("price") {
            return Err(bad("Currency price required"));
        }
        for (field, value) in values {
            if field == "generatedAt" {
                if value.as_str().is_none_or(|s| s.len() > 64) {
                    return Err(bad("Invalid price provenance"));
                }
                continue;
            }
            if !["price", "listPrice", "regulationPrice"].contains(&field.as_str()) {
                return Err(bad("Unknown currency price field"));
            }
            if value.is_null() && field != "price" {
                continue;
            }
            let m = Money::parse(
                value.as_str().ok_or(bad("Exact decimal string required"))?,
                Currency::new(code, d.scale).map_err(bad)?,
            )
            .map_err(bad)?;
            if m.minor() < 0 || m.minor() > 1_000_000_000_000 {
                return Err(bad("Currency price out of bounds"));
            }
        }
    }
    Ok(())
}
pub fn context(cfg: &Config, d: &Definition) -> Value {
    json!({"code":d.code,"scale":d.scale,"factor":d.rate,"baseCurrency":cfg.base_currency,"pricingCurrency":cfg.pricing_currency,"pricingFactor":cfg.definitions.iter().find(|v|v.code==cfg.pricing_currency).map(|v|v.rate.as_str()),"strategy":d.strategy,"rateSource":cfg.rate_source,"rateDate":cfg.rate_date})
}
