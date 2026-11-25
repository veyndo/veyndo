//! Public method discovery: the active shipping methods with converted prices and the payment methods this visitor may choose.
use crate::ports::Quotes;
use crate::service::delivery::priced_shipping;
use crate::store::cart_load;
use serde_json::{Value, json};
use veyndo_core::shop_scope::{channel, header, tenant};
use veyndo_core::{RequestContext, Result, State};
use veyndo_pricing::service::conversion as currencies;
use veyndo_shop::domain::method_text;
use veyndo_shop::service::read_cache::settings;

/// The checkout options of the request's channel, in its locale and currency.
pub async fn options(a: &State, quotes: &dyn Quotes, h: &RequestContext) -> Result<Value> {
    let t = tenant(h)?;
    let (s, revision) = settings(a, &t, channel(h)).await?;
    let cart = if header(h, "sw-context-token").is_some() {
        Some(cart_load::load(a, h).await?)
    } else {
        None
    };
    let requested = cart
        .as_ref()
        .map(|c| c.data.currency.clone())
        .unwrap_or_else(|| header(h, "x-commerce-currency").unwrap_or("").into());
    let code = s.currencies.selected(&requested)?.code.clone();
    let business = cart.as_ref().is_some_and(|c| s.is_business(&c.data.group));
    let currency = s.currencies.selected(&code)?;
    let locale = header(h, "x-commerce-locale").unwrap_or(&s.main_locale);
    let shipping = s
        .shipping
        .iter()
        .filter(|v| v.active)
        .map(|v| priced_shipping(v, locale, &s, currency))
        .collect::<Result<Vec<Value>>>()?;
    let available = quotes.available(&t, channel(h), &code).await?;
    let payments = s
        .payments
        .iter()
        .filter(|v| {
            v.active
                && (v.mode != "cash" || h.pos.is_some())
                && (!v.business_only || business)
                && v.provider.as_ref().is_none_or(|p| {
                    v.provider_method
                        .as_ref()
                        .is_some_and(|m| available.contains(&(p.clone(), m.clone())))
                })
        })
        .map(|v| method_text::localized_payment(v, locale, &s))
        .collect::<Vec<_>>();
    Ok(
        json!({"mainLocale":s.main_locale,"locales":s.locales,"countries":s.countries,"currencyContext":currencies::context(&s.currencies,currency),"shipping":shipping,"payments":payments,"revision":revision}),
    )
}
