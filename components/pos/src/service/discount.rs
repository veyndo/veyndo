//! Audited cashier discounts use exact minor units and the existing native tax allocator in both quote and commit.
use crate::service::guard;
use serde_json::{Value, json};
use veyndo_checkout::service::{app_adjustments, delivery};
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::permit;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion as currencies;
use veyndo_shop::ports::Carts;
use veyndo_shop::types::cart::{Discount, StoredCart, stored};
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
    let row = sqlx::query("SELECT * FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE")
        .bind(tenant(h)?)
        .bind(token(h)?)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(bad("Cart unavailable"))?;
    let mut c = stored(&row)?;
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
        value["actor"] = json!(h.pos.as_ref().ok_or(bad("Cashier required"))?.actor);
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
    sqlx::query("UPDATE carts SET data=$1,revision=$2 WHERE tenant=$3 AND id=$4")
        .bind(json!(c.data))
        .bind(c.revision)
        .bind(&c.tenant)
        .bind(&c.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'pos.discount_changed',$2)").bind(&c.tenant).bind(json!({"cartId":c.id,"salesChannelId":c.data.sales_channel,"cashierId":h.pos.as_ref().map(|p|&p.actor),"previous":previous,"discount":c.data.pos_discount,"revision":c.revision})).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(quote)
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
        let currency = q["price"]["currency"]
            .as_str()
            .ok_or(bad("Currency missing"))?;
        let scale = q["price"]["currencyScale"]
            .as_u64()
            .ok_or(bad("Currency scale missing"))? as u8;
        let goods = currencies::amount(
            q["price"]["positionPrice"]
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
        q["posDiscount"] = json!({"kind":d.kind,"value":d.value,"currency":d.currency,"reason":d.reason,"actor":d.actor,"minor":minor.to_string()});
    }
    Ok(q)
}
#[cfg(test)]
mod tests {
    use veyndo_shop::types::cart::Discount;
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
