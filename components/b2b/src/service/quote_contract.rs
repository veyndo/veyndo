//! Accepted offer prices feed the existing currency/tax calculator; checkout consumes the offer atomically.
use crate::store::access::{guard_conn, require};
use crate::store::purchase;
use crate::store::{carts as cart_rows, quotes};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::store::history_context as history;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_shop::ports::Carts;
use veyndo_shop::types::cart::StoredCart;
/// The JSON fields `select` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SelectInput {
    #[serde(deserialize_with = "lenient")]
    cart_revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

pub async fn select(
    carts: &dyn Carts,
    a: &State,
    h: &RequestContext,
    v: &Value,
    accept: bool,
) -> Result<Value> {
    let input: SelectInput = view(v);
    let mut c = carts.load(h).await?;
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, h, "b2b.procurement").await?;
    c = cart_rows::lock_by_id(&mut tx, &c.tenant, &c.id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    if c.status != "open" || input.cart_revision != Some(c.revision) {
        return Err(conflict("Cart changed; review before selecting the offer"));
    }
    purchase::lock_unit(&mut tx, &c).await?;
    let policy = guard_conn(&mut tx, h, &c)
        .await?
        .ok_or(bad("Select a company unit first"))?;
    require(&policy, "quotes")?;
    let context = c.data.b2b.as_mut().unwrap();
    if accept {
        let id = input.id.as_deref().ok_or(bad("Quote required"))?;
        let row = quotes::offer_locked(&mut tx, &c.tenant, id)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Quote not found".into()))?;
        if row.buyer_id != context.customer_id
            || row.unit_id != context.unit_id
            || row.cart_id != c.id
        {
            return Err(Error(
                StatusCode::NOT_FOUND,
                "Quote not found in this cart".into(),
            ));
        }
        let data: QuoteTerms = view(&row.data);
        if row.state != "offered"
            || input.revision != Some(row.revision)
            || !row.valid.unwrap_or(false)
            || data.currency != c.data.currency
            || data.items != json!(c.data.items)
        {
            return Err(conflict(
                "Offer changed, expired or no longer matches this cart",
            ));
        }
        context.quote_id = Some(id.into());
        quotes::accept(&mut tx, &c.tenant, id).await?;
    } else {
        if let Some(id) = &context.quote_id {
            let owner = quotes::CartOwner {
                cart: &c.id,
                buyer: &context.customer_id,
                unit: &context.unit_id,
            };
            quotes::release(&mut tx, &c.tenant, id, owner).await?;
        }
        context.quote_id = None;
    }
    c.revision += 1;
    cart_rows::save(&mut tx, &c.tenant, &c.id, &json!(c.data), c.revision).await?;
    crate::store::organization::event(&mut tx,&c.tenant,"b2b.quote_selected",json!({"cartId":c.id,"quoteId":c.data.b2b.as_ref().and_then(|b|b.quote_id.as_ref()),"revision":c.revision})).await?;
    tx.commit().await?;
    carts.view(&c).await
}
pub async fn prices(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    ps: &mut [veyndo_pricing::domain::catalog::Product],
) -> Result<()> {
    let Some(context) = &c.data.b2b else {
        return Ok(());
    };
    let Some(id) = &context.quote_id else {
        return Ok(());
    };
    let r = quotes::shared(conn, &c.tenant, id)
        .await?
        .ok_or(conflict("Accepted quote is missing"))?;
    let data: QuoteTerms = view(&r.data);
    if r.state != "accepted"
        || r.buyer_id != context.customer_id
        || r.unit_id != context.unit_id
        || r.cart_id != c.id
        || !r.valid.unwrap_or(false)
        || data.currency != c.data.currency
        || data.items != json!(c.data.items)
    {
        return Err(conflict(
            "Accepted offer expired or changed; release it before editing the cart",
        ));
    }
    let lines: Vec<crate::types::model::OfferLine> =
        serde_json::from_value(data.lines).map_err(|_| bad("Invalid accepted offer"))?;
    let scale = veyndo_pricing::types::currency::scale(&c.data.currency)
        .ok_or(bad("Unknown offer currency"))?;
    for p in ps {
        if let Some(line) = lines.iter().find(|l| l.id == p.id) {
            let price = line.gross_minor as f64 / 10_f64.powi(scale as i32);
            p.price = price;
            p.advanced_prices.clear();
            p.extra["priceCurrency"] = json!(c.data.currency);
            // Both fixed-currency and automatic-FX strategies resolve through their existing owner.
            p.extra["currencyPrices"] = json!({&c.data.currency:{"price":format!("{price:.precision$}",precision=scale as usize)}});
        }
    }
    Ok(())
}
pub async fn consume(conn: &mut sqlx::PgConnection, c: &StoredCart, order: &str) -> Result<()> {
    if let Some(id) = c.data.b2b.as_ref().and_then(|b| b.quote_id.as_ref())
        && !quotes::consume(conn, &c.tenant, id, order).await?
    {
        return Err(conflict("Offer has already been purchased"));
    }
    Ok(())
}

/// The terms of a quote that bind it to its cart: currency, requested items and offered lines.
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuoteTerms {
    currency: Value,
    items: Value,
    lines: Value,
}
