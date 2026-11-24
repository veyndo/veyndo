//! Tax conditions consume private authoritative pre-tax cart facts without a recursive quote.
use super::{Settings, cart_products};
use crate::foundation::App;
pub(crate) use veyndo_checkout::store::tax_conditions::tax_settings_for_cart;
use veyndo_core::Result;
use veyndo_shop::types::cart::StoredCart;
pub(crate) async fn tax_settings_for_header(
    a: &App,
    c: Option<&StoredCart>,
    chain: &[String],
    s: &Settings,
) -> Result<Settings> {
    if let Some(c) = c
        && s.taxes
            .iter()
            .any(|t| t.rules.iter().any(|r| r.condition.is_some()))
    {
        let ps = cart_products(a, &c.tenant, chain, &c.data.items).await?;
        return tax_settings_for_cart(&mut *a.db.begin().await?, c, &ps, s).await;
    }
    Ok(s.clone())
}
