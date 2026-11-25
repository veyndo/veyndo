//! Explicit document and digital acknowledgement, checked under the purchase locks and frozen into the order.
use crate::domain::quote_view::digital as is_digital;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::domain::content::translated_string;
use veyndo_core::document::view;
use veyndo_core::{Result, bad};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::verified_kernel;
use veyndo_shop::domain::buyer_consent;
use veyndo_shop::domain::legal_product::product_gaps;
use veyndo_shop::store::purchase_consent::{consent_receipt, share_acceptance};
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::legal::version;
use veyndo_shop::types::settings::Settings;

/// The cart's legal acknowledgement.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Acceptance {
    policy_version: Value,
    terms: Value,
    digital_immediate: Value,
}

/// Rejects the purchase unless the current legal acknowledgement and consent hold, then freezes them for the order.
pub async fn snapshot(
    tx: &mut sqlx::PgConnection,
    c: &StoredCart,
    s: &Settings,
    products: &[Product],
) -> Result<Value> {
    let acceptance = share_acceptance(&mut *tx, &c.tenant, &c.id)
        .await?
        .unwrap_or(json!({}));
    let consumer = !s.is_business(&c.data.group);
    let digital = consumer && products.iter().any(|p| is_digital(&p.extra));
    let accepted: Acceptance = view(&acceptance);
    if !verified_kernel::legal_checkout_admissible(
        s.legal.strict_checkout,
        accepted.policy_version == version(&s.legal),
        accepted.terms == true,
        digital,
        accepted.digital_immediate == true,
    ) {
        return Err(bad("Current legal acknowledgement required"));
    }
    if s.legal.strict_checkout {
        for key in ["privacy", "terms", "shipping"]
            .into_iter()
            .chain(consumer.then_some("withdrawal"))
        {
            if translated_string(
                &json!(s.legal.documents.get(key)),
                &c.data.locale,
                &s.main_locale,
            )
            .trim()
            .is_empty()
            {
                return Err(bad("Required legal document missing"));
            }
        }
        let gaps = products
            .iter()
            .flat_map(|p| product_gaps(&p.extra, &s.legal, &c.data.locale, &s.main_locale))
            .collect::<Vec<_>>();
        if !gaps.is_empty() {
            return Err(bad(format!(
                "Product compliance information missing: {}",
                gaps.join(", ")
            )));
        }
    }
    let consent = consent_receipt(&mut *tx, &c.tenant, &c.id, &c.data.sales_channel)
        .await?
        .filter(|r| r.policy_version == version(&s.legal) && r.fresh)
        .map(|r| r.data)
        .unwrap_or(json!({"choices":{},"decided":false}));
    buyer_consent::check(c, &consent)?;
    Ok(
        json!({"consent":consent,"buyerConsent":c.data.buyer.as_ref().and_then(|b|b.get("consent")),"policyVersion":version(&s.legal),"documents":s.legal.documents,"acceptance":acceptance,"strictCheckout":s.legal.strict_checkout,"salesChannelId":c.data.sales_channel}),
    )
}
