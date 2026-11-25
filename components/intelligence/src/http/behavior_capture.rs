//! Cart-locked behavioral capture serializes with consent withdrawal; the customer association comes from the authenticated native session.
use crate::domain::behavior_input::{Event, browser};
use crate::domain::trusted_geography::country;
use crate::service::consent::require_locked;
use crate::store::behavior as store;
use crate::store::product_reads::{cart_locked, customer_by_email};
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::shop_scope::{header, session_digest, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_customers::service::sessions::identify;
use veyndo_marketing::service::channels::admit_product;
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_shop::service::language::language_context;
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::Cart;

/// Records one consent-bound behavioral observation of the open cart.
pub async fn capture(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Event>,
) -> Result<Json<Value>> {
    v.validate()?;
    let c = load_cart(&a, &h).await?;
    if header(&h, "x-rac-channel-preview").is_some()
        || veyndo_core::store::environments::parent(&a, &c.tenant)
            .await?
            .is_some()
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sandbox and preview contexts do not record behavioral analytics".into(),
        ));
    }
    let customer = if header(&h, "x-customer-token").is_some() {
        let email = identify(&a, &tenant(&h)?, &session_digest(&h)?).await?;
        customer_by_email(&a.db, &c.tenant, &email).await?
    } else {
        None
    };
    let locale = language_context(&a, &h).await?.0;
    if let Some(product) = &v.product_id {
        admit_product(&a, &h, product).await?;
    }
    let mut tx = a.db.begin().await?;
    // Same lock order as consent withdrawal: cart -> settings -> receipt.
    let row = cart_locked(&mut *tx, &c.tenant, &c.id).await?;
    if row.status != "open" {
        return Err(conflict("Behavioral events require an open cart context"));
    }
    let current: Cart = serde_json::from_value(row.data).map_err(|e| bad(e.to_string()))?;
    require_locked(&mut tx, &c, "analytics").await?;
    let (settings, _) = scoped_locked(
        &mut tx,
        &c.tenant,
        &c.data.sales_channel,
        &validate_condition,
    )
    .await?;
    let policy = &settings.legal.behavior_analytics;
    if !policy.enabled {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Native behavioral analytics is disabled for this sales channel".into(),
        ));
    }
    if let Some(product) = &v.product_id
        && v.kind == "cart_add"
        && !current.items.iter().any(|i| i.id == *product)
    {
        return Err(bad("Cart-add observation requires an actual cart item"));
    }
    let dimensions = json!({"browser":browser(header(&h,"user-agent").unwrap_or("")),"viewport":v.viewport,"surface":v.surface,"referrerHost":v.referrer_host,"locale":locale,"country":current.checkout.as_ref().map(|s|s.country.clone()).unwrap_or("unknown".into()),"geoCountry":country(&h),"countrySource":"cart-destination"});
    let stored = store::record(
        &mut tx,
        store::Observation {
            tenant: &c.tenant,
            cart: &c.id,
            channel: &c.data.sales_channel,
            id: &v.event_id,
            kind: &v.kind,
            product: v.product_id.as_deref(),
            customer,
            dimensions,
            events_per_hour: i64::from(policy.events_per_hour),
            retention_days: policy.retention_days as i32,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(stored))
}
/// Deletes the cart's observations.
pub async fn forget(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let n = store::forget(&a, &c.tenant, &c.id).await?;
    Ok(Json(json!({"deleted":n})))
}
/// The cart's observations for the visitor.
pub async fn export(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    Ok(Json(store::export(&a, &c.tenant, &c.id).await?))
}
