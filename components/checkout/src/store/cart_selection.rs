//! The checkout choices of a cart: country, methods, contact email and addresses, priced as a candidate and written at the reviewed revision.
use crate::ports::CartEdits;
use crate::store::cart_load;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_b2b::store::access::guard_conn;
use veyndo_core::document::{Raw, lenient, view};
use veyndo_core::shop_scope::{session_digest, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_customers::service::{address_book, sessions};
use veyndo_identity::domain::credentials::email as valid_email;
use veyndo_shipping::domain::geography::validate_address;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::settings::CheckoutSelection;

/// Saved addresses replace their ids only for the authenticated owner of the cart.
async fn saved_addresses(
    a: &State,
    h: &RequestContext,
    c: &veyndo_shop::types::cart::StoredCart,
    requested: &mut CheckoutSelection,
) -> Result<()> {
    if requested.billing_address_id.is_none() && requested.shipping_address_id.is_none() {
        return Ok(());
    }
    let email = sessions::identify(a, &tenant(h)?, &session_digest(h)?).await?;
    if c.data.customer_id.is_none() || c.data.email.as_deref() != Some(&email) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Address selection requires the owning authenticated cart".into(),
        ));
    }
    let mut tx = a.db.begin().await?;
    if let Some(id) = &requested.billing_address_id {
        requested.billing_address = Some(address_book::get(&mut tx, &c.tenant, &email, id).await?);
    }
    if let Some(id) = &requested.shipping_address_id {
        let address = address_book::get(&mut tx, &c.tenant, &email, id).await?;
        requested.country = address.country.clone();
        requested.address = Some(address);
    }
    tx.commit().await?;
    Ok(())
}

/// `POST /store-api/checkout/context`: validates the requested selection, refuses one the quote cannot honour, and writes it.
/// A checkout selection edit at the cart revision it was made on.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SelectionRequest {
    #[serde(deserialize_with = "lenient")]
    checkout: Raw<SelectionAddress>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SelectionAddress {
    address: Value,
}

pub async fn select(
    a: &State,
    edits: &dyn CartEdits,
    h: &RequestContext,
    v: &Value,
) -> Result<Value> {
    let input: SelectionRequest = view(v);
    let mut requested: CheckoutSelection = serde_json::from_value(input.checkout.raw.clone())
        .map_err(|_| bad("Invalid checkout selection"))?;
    if input.checkout.address.is_object()
        && input
            .checkout
            .address
            .get("country")
            .is_none_or(Value::is_null)
        && let Some(ad) = &mut requested.address
    {
        ad.country = requested.country.clone();
    }
    // Quote a candidate before locking its cart: pricing/settings use the pool.
    // Concurrent requests must not each hold one connection while asking for another.
    let mut c = cart_load::load(a, h).await?;
    let original_revision = c.revision;
    if c.status != "open" {
        return Err(conflict("Cart is terminal"));
    }
    if input.revision != Some(c.revision) {
        return Err(conflict("Cart revision changed; reload before editing"));
    }
    saved_addresses(a, h, &c, &mut requested).await?;
    if let Some(email) = &requested.customer_email {
        let email = valid_email(&json!({"email":email}))?;
        if c.data.customer_id.is_some() && c.data.email.as_deref() != Some(&email) {
            return Err(bad("Checkout email differs from authenticated customer"));
        }
        c.data.email = Some(email);
    }
    let (settings, _) = settings(a, &c.tenant, &c.data.sales_channel).await?;
    for ad in [&mut requested.address, &mut requested.billing_address]
        .into_iter()
        .flatten()
    {
        ad.validate()?;
        validate_address(
            &ad.country,
            &ad.country_state_id,
            &settings.country_definitions,
        )?;
    }
    if requested
        .address
        .as_ref()
        .is_some_and(|a| a.country != requested.country)
    {
        return Err(bad(
            "Shipping address country differs from checkout country",
        ));
    }
    if requested.billing_address.is_none() {
        requested.billing_address = requested.address.clone();
    }
    c.data.checkout = Some(requested);
    c.revision += 1;
    let q = edits.view(&c).await?;
    if q["selectionNeedsConfirmation"] == true {
        return Err(bad("Selected country, shipping or payment is unavailable"));
    }
    let mut tx = a.db.begin().await?;
    let locked = sqlx::query!(
        "SELECT revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE",
        c.tenant,
        c.token
    )
    .fetch_one(&mut *tx)
    .await?;
    if locked.status != "open" || locked.revision != original_revision {
        return Err(conflict("Cart revision changed; reload before editing"));
    }
    guard_conn(&mut tx, h, &c).await?;
    sqlx::query!(
        "UPDATE carts SET data=$1,revision=$2 WHERE id=$3",
        json!(c.data),
        c.revision,
        c.id
    )
    .execute(&mut *tx)
    .await?;
    edits.committed(&mut tx, &c).await?;
    tx.commit().await?;
    Ok(q)
}
