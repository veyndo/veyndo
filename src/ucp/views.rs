//! Typed views of the JSON the UCP adapter reads: protocol requests, the priced native cart, stored order and consent snapshots, shop links and handoffs.
//! Every field reads exactly what indexing the JSON returned: an unexpected shape reads as the default (null, `None`, empty).
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use veyndo_core::document::{lenient, view};

/// Field deserializer that keeps a keep_present field, `null` included, as `Some`; only a missing field reads as `None`.
pub(crate) fn keep_present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// The fields of a UCP checkout or cart request the adapter reads; `buyer` stays the client's open object.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct SessionInput {
    #[serde(deserialize_with = "keep_present")]
    pub buyer: Option<Value>,
    pub line_items: Value,
    #[serde(deserialize_with = "lenient")]
    pub currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub session: Option<String>,
}

/// One requested UCP line item.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct LineInput {
    #[serde(deserialize_with = "lenient")]
    pub quantity: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    pub item: ItemRef,
}

/// The catalog reference of a requested line item.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct ItemRef {
    #[serde(deserialize_with = "lenient")]
    pub id: Option<String>,
}

/// A buyer object's consent slot: `Some` whenever the key is keep_present, also when it is `null`.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct ConsentSlot {
    #[serde(deserialize_with = "keep_present")]
    pub consent: Option<Value>,
}

/// The buyer contact the completion gate reads.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct BuyerContact {
    #[serde(deserialize_with = "lenient")]
    pub email: Option<String>,
}

impl BuyerContact {
    /// Whether the stored buyer has an email address with an `@`.
    pub fn has_email(buyer: Option<&Value>) -> bool {
        buyer
            .map(view::<Self>)
            .and_then(|b| b.email)
            .is_some_and(|s| s.contains('@'))
    }
}

/// A buyer's consent choices keyed by UCP purpose.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct BuyerConsent {
    #[serde(deserialize_with = "lenient")]
    pub consent: Map<String, Value>,
}

impl BuyerConsent {
    /// The decision for one purpose; a missing or malformed one reads as empty.
    pub fn decision(&self, purpose: &str) -> ConsentDecision {
        ConsentDecision::of(self.consent.get(purpose))
    }
}

/// One consent decision (a purpose or a marketing segment).
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct ConsentDecision {
    #[serde(deserialize_with = "lenient")]
    pub granted: Option<bool>,
    pub source: Value,
    #[serde(deserialize_with = "lenient")]
    pub segments: Map<String, Value>,
}

impl ConsentDecision {
    /// The decision held by `value`; anything but an object reads as empty.
    pub fn of(value: Option<&Value>) -> Self {
        value.map(view).unwrap_or_default()
    }
    /// The decision for one marketing segment.
    pub fn segment(&self, segment: &str) -> Self {
        Self::of(self.segments.get(segment))
    }
    /// Whether the decision explicitly grants consent.
    pub fn granted(&self) -> bool {
        self.granted == Some(true)
    }
}

/// A native consent receipt (live or from an order's legal snapshot).
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct ConsentReceipt {
    #[serde(deserialize_with = "lenient")]
    pub decided: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub choices: Map<String, Value>,
}

impl ConsentReceipt {
    /// Whether the receipt records an active decision granting the native purpose.
    pub fn active(&self, native: &str) -> bool {
        self.decided == Some(true) && self.choices.get(native) == Some(&Value::Bool(true))
    }
}

/// The stored order snapshot fields the projection reads.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct OrderSnapshot {
    pub id: Value,
    #[serde(deserialize_with = "lenient")]
    pub payment: OrderPayment,
    #[serde(deserialize_with = "lenient")]
    pub legal: OrderLegal,
}

/// The payment state of a stored order.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct OrderPayment {
    #[serde(deserialize_with = "lenient")]
    pub provider: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub attempt_id: Option<String>,
}

/// The legal snapshot an order froze at completion.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct OrderLegal {
    pub consent: Value,
    pub buyer_consent: Value,
}

/// The priced native cart (`cart_json`) fields the projection reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct PricedCart {
    #[serde(deserialize_with = "lenient")]
    pub price: PricedTotals,
    pub line_items: Value,
    pub b2b: Value,
}

/// Cart-level prices.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct PricedTotals {
    #[serde(deserialize_with = "lenient")]
    pub currency_scale: Option<i64>,
    pub currency: Value,
    #[serde(deserialize_with = "lenient")]
    pub position_price: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    pub total_price: Option<f64>,
}

/// One priced native line item.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct PricedLine {
    pub id: Value,
    pub referenced_id: Value,
    pub label: Value,
    pub quantity: Value,
    #[serde(deserialize_with = "lenient")]
    pub price: PricedLinePrice,
}

/// Line-level prices.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct PricedLinePrice {
    #[serde(deserialize_with = "lenient")]
    pub unit_price: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    pub total_price: Option<f64>,
}

/// The native purchasing constraints (`quote.b2b`) the readiness check reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Purchasing {
    #[serde(deserialize_with = "lenient")]
    pub budget_available: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub purchase_order_required: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub purchase_order: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub approval_required: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub approval: PurchasingApproval,
    #[serde(deserialize_with = "lenient")]
    pub quotes_allowed: Option<bool>,
}

/// A purchasing approval's state.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct PurchasingApproval {
    #[serde(deserialize_with = "lenient")]
    pub state: Option<String>,
}

/// The current quotes page (`b2b.quote.current`).
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct QuotePage {
    pub elements: Value,
    pub limit: Value,
}

/// A single-use checkout handoff (`checkout_handoff::issue`).
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct CheckoutHandoff {
    #[serde(deserialize_with = "lenient")]
    pub checkout_path: Option<String>,
}

/// The canonical shop links (`shop_domains::links`).
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ShopLinks {
    #[serde(deserialize_with = "lenient")]
    pub storefront_url: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub studio_url: Option<String>,
}

impl ShopLinks {
    pub fn of(tenant: &str) -> Self {
        view(&crate::shop_domains::links(tenant))
    }
    /// The canonical storefront origin; the links always carry it.
    pub fn storefront(&self) -> &str {
        self.storefront_url.as_deref().unwrap()
    }
    /// The canonical Studio origin; the links always carry it.
    pub fn studio(&self) -> &str {
        self.studio_url.as_deref().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn presence_distinguishes_missing_from_null() {
        let missing: SessionInput = view(&json!({}));
        assert!(missing.buyer.is_none());
        let null: SessionInput = view(&json!({"buyer":null}));
        assert_eq!(null.buyer, Some(Value::Null));
        let slot: ConsentSlot = view(&json!({"consent":null}));
        assert_eq!(slot.consent, Some(Value::Null));
        assert!(view::<ConsentSlot>(&json!("text")).consent.is_none());
    }

    #[test]
    fn decisions_read_like_indexing() {
        for decision in [
            json!(null),
            json!("x"),
            json!({"granted":true}),
            json!({"granted":"true","segments":[]}),
            json!({"granted":false,"source":"platform","segments":{"a":{"granted":true},"b":7}}),
        ] {
            let typed = ConsentDecision::of(Some(&decision));
            assert_eq!(typed.granted(), decision["granted"] == true);
            assert_eq!(typed.source, decision["source"]);
            for segment in ["a", "b", "c"] {
                assert_eq!(
                    typed.segment(segment).granted,
                    decision["segments"][segment]["granted"].as_bool()
                );
            }
        }
        for receipt in [
            json!(null),
            json!({"decided":true,"choices":{"analytics":true}}),
            json!({"decided":true,"choices":{"analytics":"yes"}}),
            json!({"decided":"true","choices":{"analytics":true}}),
            json!({"decided":true,"choices":[true]}),
        ] {
            let typed: ConsentReceipt = view(&receipt);
            assert_eq!(
                typed.active("analytics"),
                receipt["decided"] == true && receipt["choices"]["analytics"] == true
            );
        }
    }

    #[test]
    fn purchasing_and_prices_read_like_indexing() {
        let p = json!({"budgetAvailable":false,"approval":{"state":7},"quotesAllowed":true,"purchaseOrder":" "});
        let typed: Purchasing = view(&p);
        assert_eq!(typed.budget_available, Some(false));
        assert_eq!(typed.approval.state, None);
        assert_eq!(typed.quotes_allowed, Some(true));
        assert_eq!(typed.purchase_order.as_deref(), Some(" "));
        let q = json!({"price":{"currencyScale":3,"currency":"EUR","positionPrice":1,"totalPrice":"2"},"lineItems":[]});
        let typed: PricedCart = view(&q);
        assert_eq!(typed.price.currency_scale, Some(3));
        assert_eq!(typed.price.position_price, Some(1.0));
        assert_eq!(typed.price.total_price, None);
        assert_eq!(typed.price.currency, json!("EUR"));
    }
}
