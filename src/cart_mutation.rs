//! Optimistic cart mutations and quantity normalization.
use crate::cart_price::Quotes;
use crate::cart_price::cart_json;
use crate::foundation::App;
use crate::{agent_runs, apps, legal};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use veyndo_apps::sandbox::Sandbox;
use veyndo_checkout::store::cart_edit::{self, Edit};
use veyndo_core::Result;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_shop::types::cart::Item;
use veyndo_shop::types::cart::StoredCart;

pub(crate) async fn set_items(
    a: &App,
    h: &RequestContext,
    items: Vec<Item>,
    expected: Option<i64>,
) -> Result<StoredCart> {
    set_cart(a, h, items, expected, None).await
}
pub(crate) async fn set_cart(
    a: &App,
    h: &RequestContext,
    items: Vec<Item>,
    expected: Option<i64>,
    buyer: Option<Option<Value>>,
) -> Result<StoredCart> {
    set_configured_cart(a, h, items, expected, buyer, None).await
}
pub(crate) async fn set_configured_cart(
    a: &App,
    h: &RequestContext,
    items: Vec<Item>,
    expected: Option<i64>,
    buyer: Option<Option<Value>>,
    selections: Option<&[apps::ConfigurationSelection]>,
) -> Result<StoredCart> {
    let edit = Edit {
        items,
        expected,
        buyer,
        ..Edit::default()
    };
    commit(a, h, edit, Edits::new(a, selections, None)).await
}

pub(crate) async fn reviewed_cart(
    a: &App,
    h: &RequestContext,
    items: Vec<Item>,
    revision: i64,
    review: &str,
) -> Result<StoredCart> {
    let edit = Edit {
        items,
        expected: Some(revision),
        review: Some(review),
        ..Edit::default()
    };
    commit(a, h, edit, Edits::new(a, None, None)).await
}

/// The quote parts an item preview reports, each exactly as quoted.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PreviewQuote {
    line_items: Value,
    price: Value,
    #[serde(deserialize_with = "lenient")]
    checkout: PreviewCheckout,
    shipping_costs: Value,
    #[serde(deserialize_with = "lenient")]
    deliveries: Vec<Value>,
}

/// The destination of a quoted cart.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PreviewCheckout {
    country: Value,
}

/// The first quoted delivery: its date and method name.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PreviewDelivery {
    delivery_date: Value,
    #[serde(deserialize_with = "lenient")]
    shipping_method: PreviewMethod,
}

/// The name of a quoted shipping method.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PreviewMethod {
    name: Value,
}

pub(crate) async fn preview_items(
    a: &App,
    h: &RequestContext,
    items: Vec<Item>,
    revision: i64,
) -> Result<Value> {
    let c = cart_edit::candidate(a, &Quotes(a), h, items, Some(revision)).await?;
    let q = cart_json(a, &c).await?;
    let quote: PreviewQuote = view(&q);
    let first = quote
        .deliveries
        .first()
        .map(view::<PreviewDelivery>)
        .unwrap_or_default();
    Ok(
        json!({"revision":c.revision,"review":cart_edit::review_digest(&c,&q),
        "lineItems":quote.line_items,"price":quote.price,"country":quote.checkout.country,
        "shippingCosts":quote.shipping_costs,
        "delivery":first.delivery_date,
        "shippingMethod":first.shipping_method.name}),
    )
}

/// Protocol consent and cart contents commit together through the native owners.
pub(crate) async fn set_consented_cart(
    a: &App,
    h: &RequestContext,
    items: Vec<Item>,
    expected: i64,
    buyer: Option<Value>,
    consent: Option<(String, HashMap<String, bool>)>,
    currency: Option<&str>,
) -> Result<StoredCart> {
    let edit = Edit {
        items,
        expected: Some(expected),
        buyer: Some(buyer),
        currency,
        review: None,
    };
    commit(a, h, edit, Edits::new(a, None, consent)).await
}

async fn commit(
    a: &App,
    h: &RequestContext,
    edit: Edit<'_>,
    edits: Edits<'_>,
) -> Result<StoredCart> {
    cart_edit::commit(a, &Quotes(a), &edits, h, edit).await
}

type Modules = HashMap<String, Arc<Sandbox>>;
type Consent = (String, HashMap<String, bool>);

/// The server side of one cart edit: the app selections it carries, its consent, and what it prepared on the way.
pub(crate) struct Edits<'a> {
    app: &'a App,
    selections: Option<&'a [apps::ConfigurationSelection]>,
    consent: Mutex<Option<Consent>>,
    before: Mutex<Option<HashMap<String, veyndo_shop::types::cart::AppConfiguration>>>,
    modules: Mutex<Option<Modules>>,
}
impl<'a> Edits<'a> {
    pub(crate) fn new(
        app: &'a App,
        selections: Option<&'a [apps::ConfigurationSelection]>,
        consent: Option<Consent>,
    ) -> Self {
        Edits {
            app,
            selections,
            consent: Mutex::new(consent),
            before: Mutex::new(None),
            modules: Mutex::new(None),
        }
    }
}
fn take<T>(slot: &Mutex<Option<T>>) -> Option<T> {
    slot.lock().unwrap_or_else(|e| e.into_inner()).take()
}
fn put<T>(slot: &Mutex<Option<T>>, value: T) {
    *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
}
impl veyndo_checkout::ports::CartEdits for Edits<'_> {
    fn view<'a>(&'a self, cart: &'a StoredCart) -> veyndo_core::PortFuture<'a, Value> {
        Box::pin(cart_json(self.app, cart))
    }
    fn configure<'a>(&'a self, cart: &'a mut StoredCart) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            let Some(selections) = self.selections else {
                return Ok(());
            };
            put(&self.before, cart.data.app_configurations.clone());
            cart.data.app_configurations =
                apps::resolve_selections(self.app, cart, selections).await?;
            apps::configuration_budget(&cart.data.app_configurations)?;
            put(
                &self.modules,
                apps::prepare_configurations(self.app, cart).await?,
            );
            Ok(())
        })
    }
    fn locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            if let Some(modules) = take(&self.modules) {
                apps::validate_configurations(tx, cart, &modules).await?;
            }
            Ok(())
        })
    }
    fn committed<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        cart: &'a StoredCart,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            if let Some(before) = take(&self.before) {
                apps::configuration_events(tx, cart, &before).await?;
            }
            if let Some((policy, choices)) = take(&self.consent) {
                legal::consent_locked(tx, cart, &policy, choices, true).await?;
            }
            agent_runs::checkpoint::fence_commit(self.app, tx).await
        })
    }
}
