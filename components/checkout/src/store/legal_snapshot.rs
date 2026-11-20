//! Explicit document and digital acknowledgement, checked under the purchase locks and frozen into the order.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::domain::content::translated_string;
use veyndo_core::{Result, bad};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::verified_kernel;
use veyndo_shop::domain::buyer_consent;
use veyndo_shop::domain::legal_product::product_gaps;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::legal::version;
use veyndo_shop::types::settings::Settings;

// Aliases are server-created, one hop and immutable; both sides are tenant/channel scoped.
pub const RECEIPT_SQL: &str = "SELECT root.cart_id,root.policy_version,root.data,root.expires_at>now() AS fresh FROM privacy_consents own JOIN privacy_consents root ON root.tenant=own.tenant AND root.channel_id=own.channel_id AND root.cart_id=coalesce(own.source_cart_id,own.cart_id) AND root.source_cart_id IS NULL WHERE own.tenant=$1 AND own.cart_id=$2 AND own.channel_id=$3 FOR SHARE OF root";

/// Rejects the purchase unless the current legal acknowledgement and consent hold, then freezes them for the order.
pub async fn snapshot(
    tx: &mut sqlx::PgConnection,
    c: &StoredCart,
    s: &Settings,
    products: &[Product],
) -> Result<Value> {
    let acceptance: Value = sqlx::query_scalar(
        "SELECT data FROM legal_acceptances WHERE tenant=$1 AND cart_id=$2 FOR SHARE",
    )
    .bind(&c.tenant)
    .bind(&c.id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(json!({}));
    let consumer = !s.is_business(&c.data.group);
    let digital = consumer && products.iter().any(|p| p.extra["digital"] == true);
    if !verified_kernel::legal_checkout_admissible(
        s.legal.strict_checkout,
        acceptance["policyVersion"] == version(&s.legal),
        acceptance["terms"] == true,
        digital,
        acceptance["digitalImmediate"] == true,
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
    let consent = sqlx::query(RECEIPT_SQL)
        .bind(&c.tenant)
        .bind(&c.id)
        .bind(&c.data.sales_channel)
        .fetch_optional(&mut *tx)
        .await?
        .filter(|r| {
            r.get::<String, _>("policy_version") == version(&s.legal) && r.get::<bool, _>("fresh")
        })
        .map(|r| r.get::<Value, _>("data"))
        .unwrap_or(json!({"choices":{},"decided":false}));
    buyer_consent::check(c, &consent)?;
    Ok(
        json!({"consent":consent,"buyerConsent":c.data.buyer.as_ref().and_then(|b|b.get("consent")),"policyVersion":version(&s.legal),"documents":s.legal.documents,"acceptance":acceptance,"strictCheckout":s.legal.strict_checkout,"salesChannelId":c.data.sales_channel}),
    )
}
