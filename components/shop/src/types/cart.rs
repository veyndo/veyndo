//! The cart as every commerce component reads it: the shared cart contract bound to this shop's contexts.
use crate::types::settings::CheckoutSelection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::PgRow;
use std::collections::HashMap;
use veyndo_core::document::{merge, view};
use veyndo_core::{Result, bad};
pub use veyndo_pricing::cart::Item;
use veyndo_pricing::verified_kernel;

pub type Cart =
    veyndo_pricing::cart::Cart<Discount, BusinessContext, AppConfiguration, CheckoutSelection>;

/// A cart row with its token, optimistic revision and lifecycle status.
#[derive(Clone)]
pub struct StoredCart {
    pub id: String,
    pub tenant: String,
    pub token: String,
    pub data: Cart,
    pub revision: i64,
    pub status: String,
}

/// Hydrates a complete `carts` row; older stored shapes are upgraded on the way.
pub fn stored(r: &PgRow) -> Result<StoredCart> {
    CartRow {
        id: r.get("id"),
        tenant: r.get("tenant"),
        token: r.get("token"),
        data: r.get("data"),
        revision: r.get("revision"),
        status: r.get("status"),
    }
    .hydrate()
}

/// The columns of a `carts` row as a compile-checked query selects them (`SELECT id,tenant,token,data,revision,status`).
pub struct CartRow {
    pub id: String,
    pub tenant: String,
    pub token: String,
    pub data: Value,
    pub revision: i64,
    pub status: String,
}

impl CartRow {
    /// The stored cart; older stored shapes are upgraded on the way.
    pub fn hydrate(self) -> Result<StoredCart> {
        let mut data = self.data;
        upgrade_cart(&mut data);
        Ok(StoredCart {
            id: self.id,
            tenant: self.tenant,
            token: self.token,
            data: serde_json::from_value(data).map_err(|e| bad(e.to_string()))?,
            revision: self.revision,
            status: self.status,
        })
    }
}

/// The checkout choices of a cart; a cart without any starts from the defaults.
pub fn selection(c: &Cart) -> CheckoutSelection {
    c.checkout
        .clone()
        .unwrap_or_else(CheckoutSelection::defaults)
}

/// Rewrites app configurations stored before apps were named, without touching anything else.
pub fn upgrade_cart(data: &mut Value) {
    let Some(values) = data
        .get_mut("app_configurations")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for (product, value) in values {
        if value.get("app").is_none() {
            let legacy: LegacyConfiguration = view(value);
            // A configuration without an app is null or an object, as indexing assumed; merge rejects anything else unchanged.
            let _ = merge(
                value,
                json!({"app":"engraving","productId":product,"fields":{"text":legacy.text},"entity":"rules","record":"default"}),
            );
        }
    }
}

/// The engraving text of an app configuration stored before apps were named.
#[derive(Deserialize, Default)]
#[serde(default)]
struct LegacyConfiguration {
    text: Value,
}

/// A cashier's audited discount on a point-of-sale cart.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Discount {
    pub kind: String,
    pub value: u64,
    pub currency: String,
    pub reason: String,
    pub actor: String,
}
impl Discount {
    /// The discount in minor units of `currency` on goods worth `goods`; `actor` says a cashier is present.
    pub fn minor(&self, goods: u64, currency: &str, actor: bool) -> Result<i64> {
        if self.currency != currency || goods > i64::MAX as u64 {
            return Err(bad("Discount currency changed; remove and apply again"));
        }
        let minor = match self.kind.as_str() {
            "percent" if self.value <= 10000 => {
                ((u128::from(goods) * u128::from(self.value) + 5000) / 10000) as u64
            }
            "fixed" => self.value,
            _ => {
                return Err(bad(
                    "Discount requires basis points from 0 to 10000 or exact minor units",
                ));
            }
        };
        if !verified_kernel::pos_discount_admissible(
            actor,
            self.currency == currency,
            !self.reason.trim().is_empty(),
            minor,
            goods,
        ) {
            return Err(bad("Discount exceeds the current goods value"));
        }
        Ok(minor as i64)
    }
}

/// The purchasing unit and employee a company cart acts for.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessContext {
    pub unit_id: String,
    pub customer_id: String,
    #[serde(default)]
    pub purchase_order: String,
    #[serde(default)]
    pub quote_id: Option<String>,
}

/// What an app contributed to a cart line: the buyer's inputs, a fee and how to show them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfiguration {
    pub app: String,
    pub product_id: String,
    pub fields: Value,
    pub fee_minor: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_fields: Option<Value>,
    pub rule_revision: i64,
    pub app_version: String,
    pub entity: String,
    pub record: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<Snapshot>,
}

/// How an app configuration is shown on carts, orders and documents.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub title: HashMap<String, String>,
    pub main_locale: String,
    pub fields: Vec<DisplayField>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayField {
    pub label: HashMap<String, String>,
    pub value: DisplayValue,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DisplayValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
    Localized(HashMap<String, String>),
}

#[cfg(test)]
mod tests {
    use super::upgrade_cart;
    use serde_json::json;
    #[test]
    fn legacy_cart_adapter_is_explicit_and_idempotent() {
        let mut cart = json!({"app_configurations":{"mug":{"text":"Ada","feeMinor":300,"ruleRevision":1,"appVersion":"1.0.0"}},"order":{"immutable":"snapshot"}});
        upgrade_cart(&mut cart);
        assert_eq!(
            cart["app_configurations"]["mug"]["fields"],
            json!({"text":"Ada"})
        );
        let snapshot = cart.clone();
        upgrade_cart(&mut cart);
        assert_eq!(cart, snapshot);
        assert_eq!(cart["order"], json!({"immutable":"snapshot"}));
    }
}
