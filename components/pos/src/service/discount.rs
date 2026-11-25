//! Audited cashier discounts use exact minor units and the existing native tax allocator in both quote and commit.
use crate::service::guard;
use crate::store::carts as cart_rows;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_checkout::service::{app_adjustments, delivery};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::permit;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion as currencies;
use veyndo_shop::ports::Carts;
use veyndo_shop::types::cart::{Discount, StoredCart};
use veyndo_shop::types::settings::Settings;
pub async fn save(
    carts: &dyn Carts,
    a: &State,
    h: &RequestContext,
    v: &Value,
    revision: i64,
) -> Result<Value> {
    permit(h, "orders.write")?;
    permit(h, "payments.manage")?;
    let mut tx = a.db.begin().await?;
    let mut c = cart_rows::lock(&mut tx, &tenant(h)?, token(h)?)
        .await?
        .ok_or(bad("Cart unavailable"))?;
    guard::cart(h, &c)?;
    if c.data.pos_actor.is_none() || c.status != "open" || revision != c.revision {
        return Err(conflict("POS sale changed; reload before discounting"));
    }
    let previous = c.data.pos_discount.clone();
    c.data.pos_discount = if v.is_null() {
        None
    } else {
        if !v.is_object() {
            return Err(bad("Cashier discount must be an object or null"));
        }
        let mut value = v.clone();
        merge(
            &mut value,
            Actor {
                actor: &h.pos.as_ref().ok_or(bad("Cashier required"))?.actor,
            },
        )?;
        let d: Discount =
            serde_json::from_value(value).map_err(|_| bad("Invalid cashier discount"))?;
        if d.reason.trim().is_empty()
            || d.reason.len() > 240
            || d.reason.chars().any(char::is_control)
        {
            return Err(bad("Discount reason required (1..240 characters)"));
        }
        Some(d)
    };
    c.revision += 1;
    let quote = carts.view(&c).await?;
    cart_rows::save(&mut tx, &json!(c.data), c.revision, &c.tenant, &c.id).await?;
    veyndo_core::store::outbox::publish_on(&mut *tx, &c.tenant, "pos.discount_changed", &json!({"cartId":c.id,"salesChannelId":c.data.sales_channel,"cashierId":h.pos.as_ref().map(|p|&p.actor),"previous":previous,"discount":c.data.pos_discount,"revision":c.revision})).await?;
    tx.commit().await?;
    Ok(quote)
}
/// The quote's goods price a cashier discount is taken from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuotedGoods {
    #[serde(deserialize_with = "lenient")]
    price: GoodsPrice,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct GoodsPrice {
    currency: Value,
    currency_scale: Value,
    position_price: Value,
}

pub fn apply(
    c: &StoredCart,
    mut q: Value,
    ps: &[Product],
    settings: &Settings,
    revision: i64,
) -> Result<Value> {
    if let Some(d) = &c.data.pos_discount {
        if c.data.pos_actor.as_deref() != Some(&d.actor) {
            return Err(bad("Cashier discount requires its admitted POS cart"));
        }
        let price = view::<QuotedGoods>(&q).price;
        let currency = price.currency.as_str().ok_or(bad("Currency missing"))?;
        let scale = price
            .currency_scale
            .as_u64()
            .ok_or(bad("Currency scale missing"))? as u8;
        let goods = currencies::amount(
            price
                .position_price
                .as_f64()
                .ok_or(bad("Goods total missing"))?,
            currency,
            scale,
        )?
        .minor();
        let minor = d.minor(
            u64::try_from(goods).map_err(|_| bad("Negative goods total"))?,
            currency,
            c.data.pos_actor.as_deref() == Some(&d.actor),
        )?;
        app_adjustments::apply(&mut q, "discount", minor)?;
        q = delivery::enrich(q, c, ps, settings, revision)?;
        merge(
            &mut q,
            json!({"posDiscount":{"kind":d.kind,"value":d.value,"currency":d.currency,"reason":d.reason,"actor":d.actor,"minor":minor.to_string()}}),
        )?;
    }
    Ok(q)
}
/// The cashier a discount is recorded for.
#[derive(serde::Serialize)]
struct Actor<'a> {
    actor: &'a str,
}

#[cfg(test)]
mod tests {
    use super::Actor;
    use serde_json::json;
    use veyndo_core::document::merge;
    use veyndo_shop::types::cart::Discount;

    #[test]
    fn the_cashier_is_recorded_as_indexing_recorded_it() {
        let value =
            json!({"kind":"fixed","value":100,"currency":"EUR","reason":"r","actor":"spoofed"});
        let mut old = value.clone();
        old["actor"] = json!("cashier");
        let mut new = value;
        merge(&mut new, Actor { actor: "cashier" }).unwrap();
        assert_eq!(new, old);
    }
    fn d(kind: &str, value: u64) -> Discount {
        Discount {
            kind: kind.into(),
            value,
            currency: "EUR".into(),
            reason: "Damaged packaging".into(),
            actor: "cashier".into(),
        }
    }
    #[test]
    fn exact_bounds_and_currency_fence() {
        assert_eq!(d("percent", 1000).minor(999, "EUR", true).unwrap(), 100);
        assert_eq!(d("percent", 10000).minor(999, "EUR", true).unwrap(), 999);
        assert_eq!(d("fixed", 100).minor(999, "EUR", true).unwrap(), 100);
        for (kind, value) in [("fixed", 1000), ("percent", 10001), ("other", 1)] {
            assert!(d(kind, value).minor(999, "EUR", true).is_err());
        }
        assert!(d("fixed", 1).minor(999, "JPY", true).is_err());
        assert!(d("fixed", 1).minor(999, "EUR", false).is_err());
        for goods in [0, 1, 9, 99, 999, 100_000_000] {
            for basis in [0, 1, 500, 3333, 9999, 10000] {
                let n = d("percent", basis).minor(goods, "EUR", true).unwrap();
                assert!(n >= 0 && n as u64 <= goods);
            }
        }
    }
}
