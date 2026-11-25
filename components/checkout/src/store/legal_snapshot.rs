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
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::legal::version;
use veyndo_shop::types::settings::Settings;

// Aliases are server-created, one hop and immutable; both sides are tenant/channel scoped.
pub const RECEIPT_SQL: &str = "SELECT root.cart_id,root.policy_version,root.data,root.expires_at>now() AS fresh FROM privacy_consents own JOIN privacy_consents root ON root.tenant=own.tenant AND root.channel_id=own.channel_id AND root.cart_id=coalesce(own.source_cart_id,own.cart_id) AND root.source_cart_id IS NULL WHERE own.tenant=$1 AND own.cart_id=$2 AND own.channel_id=$3 FOR SHARE OF root";

/// The cart's legal acknowledgement.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Acceptance {
    policy_version: Value,
    terms: Value,
    digital_immediate: Value,
}

/// The root consent receipt a cart's consent resolves to (`RECEIPT_SQL`), share-locked.
pub struct ConsentReceipt {
    pub cart_id: String,
    pub policy_version: String,
    pub data: Value,
    /// The receipt has not expired.
    pub fresh: bool,
}

/// The cart's consent receipt in the channel (through its one-hop alias), share-locked; the same statement as `RECEIPT_SQL`, checked at compile time.
pub async fn consent_receipt(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    cart: &str,
    channel: &str,
) -> Result<Option<ConsentReceipt>> {
    Ok(sqlx::query_as!(
        ConsentReceipt,
        r#"SELECT root.cart_id,root.policy_version,root.data,root.expires_at>now() AS "fresh!" FROM privacy_consents own JOIN privacy_consents root ON root.tenant=own.tenant AND root.channel_id=own.channel_id AND root.cart_id=coalesce(own.source_cart_id,own.cart_id) AND root.source_cart_id IS NULL WHERE own.tenant=$1 AND own.cart_id=$2 AND own.channel_id=$3 FOR SHARE OF root"#,
        tenant,
        cart,
        channel
    )
    .fetch_optional(tx)
    .await?)
}

/// Rejects the purchase unless the current legal acknowledgement and consent hold, then freezes them for the order.
pub async fn snapshot(
    tx: &mut sqlx::PgConnection,
    c: &StoredCart,
    s: &Settings,
    products: &[Product],
) -> Result<Value> {
    let acceptance: Value = sqlx::query_scalar!(
        "SELECT data FROM legal_acceptances WHERE tenant=$1 AND cart_id=$2 FOR SHARE",
        c.tenant,
        c.id
    )
    .fetch_optional(&mut *tx)
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
