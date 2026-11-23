//! Optimistic cart edits: normalized quantities, the expected revision, an optional review fingerprint and the locked write.
use crate::domain::cart_quote::normalized_quantity;
use crate::ports::{CartEdits, Quotes};
use crate::store::cart_load;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use veyndo_b2b::store::access::guard_conn;
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_marketing::service::channels::admit_product;
use veyndo_shop::domain::cart_lines::validate as validate_items;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::cart::{Item, StoredCart};

/// One edit of the request's cart; everything but the lines is optional.
#[derive(Default)]
pub struct Edit<'a> {
    pub items: Vec<Item>,
    pub expected: Option<i64>,
    pub buyer: Option<Option<Value>>,
    pub currency: Option<&'a str>,
    pub review: Option<&'a str>,
}

/// The fingerprint of a reviewed quote: a precondition for a conditional edit, never a credential or a source of prices.
pub fn review_digest(c: &StoredCart, quote: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                c.id,
                c.revision,
                quote["lineItems"],
                quote["price"],
                quote["checkout"],
                quote["shippingCosts"],
                quote["deliveries"]
            ])
            .to_string()
        )
    )
}

/// The cart with the new lines and normalized quantities, checked before any cart lock is taken.
pub async fn candidate(
    a: &State,
    quotes: &dyn Quotes,
    h: &RequestContext,
    items: Vec<Item>,
    expected: Option<i64>,
) -> Result<StoredCart> {
    validate_items(&items)?;
    for i in &items {
        admit_product(a, h, &i.id).await?;
    }
    // Validate the candidate before taking a cart lock. These helpers use the
    // pool: holding one connection per edit while requesting another can exhaust
    // the pool and deadlock concurrent edits. The revision is checked again below.
    let mut c = cart_load::load(a, h).await?;
    if c.status != "open" {
        return Err(conflict("Cart is terminal"));
    }
    if expected.is_some_and(|v| v != c.revision) {
        return Err(conflict("Cart revision changed; reload before editing"));
    }
    let (locale, chain) = language_context(a, h).await?;
    let ps = quotes.products(&c.tenant, &chain, &items).await?;
    c.data.items = items
        .into_iter()
        .map(|mut item| {
            let p = ps
                .iter()
                .find(|p| p.id == item.id)
                .ok_or(bad("Unknown product"))?;
            item.quantity = normalized_quantity(p, item.quantity)?;
            Ok(item)
        })
        .collect::<Result<Vec<_>>>()?;
    c.data.locale = locale;
    Ok(c)
}

/// Writes the edit when the cart is still open at the expected revision and the reviewed quote still holds.
pub async fn commit(
    a: &State,
    quotes: &dyn Quotes,
    edits: &dyn CartEdits,
    h: &RequestContext,
    edit: Edit<'_>,
) -> Result<StoredCart> {
    let mut c = candidate(a, quotes, h, edit.items, edit.expected).await?;
    if let Some(currency) = edit.currency {
        let (settings, _) = settings(a, &c.tenant, &c.data.sales_channel).await?;
        c.data.currency = settings.currencies.selected(currency)?.code.clone();
    }
    if let Some(b) = edit.buyer {
        c.data.buyer = b;
    }
    edits.configure(&mut c).await?;
    let quote = edits.view(&c).await?;
    if edit
        .review
        .is_some_and(|value| value != review_digest(&c, &quote))
    {
        return Err(conflict(
            "Cart quote changed; review price and delivery again",
        ));
    }
    let mut tx = a.db.begin().await?;
    let locked =
        sqlx::query("SELECT revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE")
            .bind(&c.tenant)
            .bind(&c.token)
            .fetch_one(&mut *tx)
            .await?;
    if locked.get::<String, _>("status") != "open" {
        return Err(conflict("Cart is terminal"));
    }
    if locked.get::<i64, _>("revision") != c.revision {
        return Err(conflict("Cart revision changed; reload before editing"));
    }
    guard_conn(&mut tx, h, &c).await?;
    edits.locked(&mut tx, &c).await?;
    c.revision += 1;
    sqlx::query("UPDATE carts SET data=$1,revision=$2 WHERE id=$3")
        .bind(json!(c.data))
        .bind(c.revision)
        .bind(&c.id)
        .execute(&mut *tx)
        .await?;
    edits.committed(&mut tx, &c).await?;
    tx.commit().await?;
    Ok(c)
}
