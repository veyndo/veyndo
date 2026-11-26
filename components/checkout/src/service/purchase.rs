//! The purchase transaction: native preparation, ordered locks with replay admission, the authoritative locked quote, and one atomic order commit; no external provider call.
mod commit;
mod lock;
mod price;

use crate::domain::session_review::Review;
use crate::ports::Purchase;
use crate::store::cart_load;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_marketing::service::channels::admit_product;
use veyndo_pricing::domain::money::Money;
use veyndo_shop::Settings;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::settings::CheckoutSelection;

/// What preparation admitted before any lock: the cart, its app modules and its language chains.
struct Prepared<P> {
    admitted: StoredCart,
    modules: P,
    language_chain: Vec<String>,
    locale_chain: Vec<String>,
}
/// The locked purchase: its transaction, the locked cart and the purchase fingerprint.
struct Locked<P> {
    tx: Transaction<'static, Postgres>,
    cart: StoredCart,
    prepared: Prepared<P>,
    fingerprint: String,
}
/// The authoritative quote with the settings, total and legal snapshot it was computed under.
struct Priced {
    q: Value,
    config: Settings,
    money: Money,
    minor: i64,
    selected: CheckoutSelection,
    legal_snapshot: Value,
}
/// A placed order with its cart and the payment method that paid it.
pub struct Placed {
    pub order: Value,
    pub cart: StoredCart,
    pub method: String,
}
/// Either a purchase that may proceed, or the order an earlier identical request already placed.
enum Admission<L> {
    Active(L),
    Replay(Value),
}

/// Admits the cart, its products and its app modules before the transaction holds a pool connection.
async fn prepare<P: Purchase>(
    a: &State,
    port: &P,
    h: &RequestContext,
    key: &str,
) -> Result<Prepared<P::Prepared>> {
    if key.len() < 8 || key.len() > 128 {
        return Err(bad("Idempotency-Key must contain 8..128 characters"));
    }
    let admitted = cart_load::load(a, h).await?;
    let (initial, _) = settings(a, &admitted.tenant, "default").await?;
    let modules = port
        .prepare(&admitted, initial.is_business(&admitted.data.group))
        .await?;
    for i in &admitted.data.items {
        admit_product(a, h, &i.id).await?;
    }
    let (language_chain, locale_chain) = crate::service::language::language(a, &admitted).await?;
    Ok(Prepared {
        admitted,
        modules,
        language_chain,
        locale_chain,
    })
}

/// The outcome of a purchase request.
pub enum Outcome {
    /// A new order was placed in this request.
    Placed(Box<Placed>),
    /// An identical earlier request already placed this order.
    Replay(Value),
}

/// Places the order of the request's cart once per idempotency key; a repeated key returns the first order.
pub async fn execute<P: Purchase>(
    a: &State,
    port: &P,
    h: &RequestContext,
    key: &str,
    review: Option<&Review>,
) -> Result<Outcome> {
    let prepared = prepare(a, port, h, key).await?;
    let mut locked = match lock::run(a, h, key, prepared, review).await? {
        Admission::Replay(order) => return Ok(Outcome::Replay(order)),
        Admission::Active(locked) => locked,
    };
    let priced = price::run(port, h, &mut locked).await?;
    Ok(Outcome::Placed(Box::new(
        commit::run(port, h, key, locked, priced, review).await?,
    )))
}
