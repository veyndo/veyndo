//! Customer accounts on the storefront: registration, the typed profile and the password swap; price groups, email and merchant roles cannot be self-assigned.
use crate::service::address_entries::address_list;
use crate::service::directory::decorate;
use crate::service::sessions::{identify, open};
use crate::store::accounts;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::shop_scope::{channel as channel_id, session_digest, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_identity::domain::credentials::{
    email, hash_password, name, password as new_password, verify_password,
};
use veyndo_marketing::service::channels;
use veyndo_pricing::customer::{Address, Contact};
use veyndo_shipping::domain::geography::validate_address;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;

/// The signed-in customer of the request and their shop.
async fn customer(a: &State, h: &RequestContext) -> Result<(String, String)> {
    let t = tenant(h)?;
    let email = identify(a, &t, &session_digest(h)?).await?;
    Ok((t, email))
}

/// The contact fields of a customer registration; a missing or non-text field is empty.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Registration {
    #[serde(deserialize_with = "lenient")]
    first_name: String,
    #[serde(deserialize_with = "lenient")]
    last_name: String,
    #[serde(deserialize_with = "lenient")]
    company: String,
    #[serde(deserialize_with = "lenient")]
    phone_number: String,
}

/// A customer password change.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PasswordChange {
    #[serde(deserialize_with = "lenient")]
    old_password: Option<String>,
    new_password: Value,
}

/// `POST /store-api/account/register`: a customer with contact and optional first addresses, signed in at once.
pub async fn register(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = tenant(&h)?;
    let email = email(&v)?;
    let name = name(&v)?;
    let password = hash_password(new_password(&v)?).await?;
    let exists: bool = veyndo_core::store::tenants::exists(&a.db, &t).await?;
    if !exists {
        return Err(bad("Unknown shop"));
    }
    let input: Registration = view(&v);
    let mut contact: Contact = serde_json::from_value(json!({"name":name,"firstName":input.first_name,"lastName":input.last_name,"company":input.company,"phoneNumber":input.phone_number})).map_err(|_|bad("Invalid customer contact"))?;
    contact.validate()?;
    let mut initial = Vec::new();
    let (settings, _) = settings(&a, &t, "default").await?;
    for field in ["billingAddress", "shippingAddress"] {
        if !v[field].is_null() {
            let mut ad: Address = serde_json::from_value(v[field].clone())
                .map_err(|_| bad("Invalid registration address"))?;
            ad.validate()?;
            validate_address(
                &ad.country,
                &ad.country_state_id,
                &settings.country_definitions,
            )?;
            if !settings.countries.contains(&ad.country) {
                return Err(bad("Address country unavailable"));
            }
            initial.push(accounts::Initial {
                address: ad,
                default_billing: field == "billingAddress",
                default_shipping: field == "shippingAddress",
            });
        }
    }
    let locale = language_context(&a, &h).await?.0;
    let channel = channel_id(&h);
    channels::channel(&a, &h, channel, &locale).await?;
    accounts::register(
        &a, &t, &email, password, &contact, initial, &locale, channel,
    )
    .await?;
    Ok(Json(
        json!({"customerToken":open(&a,&t,&email).await?,"email":email}),
    ))
}
/// `GET /store-api/account/profile`: the customer with addresses and standard read fields.
pub async fn get(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let (t, email) = customer(&a, &h).await?;
    let mut v = json!(accounts::row(&a, &t, &email).await?);
    merge(
        &mut v,
        json!({"addresses": address_list(&a, &t, &email).await?}),
    )?;
    decorate(&a, &t, &email, &mut v).await?;
    Ok(Json(v))
}
/// `PUT /store-api/account/profile`: contact fields only; groups, email and roles cannot be self-assigned.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (t, email) = customer(&a, &h).await?;
    let mut p: Contact = serde_json::from_value(v)
        .map_err(|_| bad("Only customer contact fields can be updated"))?;
    p.validate()?;
    if let Some(id) = &p.default_payment_method_id {
        let (s, _) = settings(&a, &t, "default").await?;
        let group = accounts::group(&a, &t, &email).await?;
        if !s
            .payments
            .iter()
            .any(|m| m.id == *id && m.active && (!m.business_only || s.is_business(&group)))
        {
            return Err(bad("Payment method unavailable for customer"));
        }
    }
    if let Some(ad) = &p.address {
        let (settings, _) = settings(&a, &t, "default").await?;
        if !settings.countries.contains(&ad.country) {
            return Err(bad("Address country unavailable"));
        }
    }
    accounts::save_profile(&a, &t, &email, &p).await?;
    Ok(Json(json!({"saved":true})))
}
/// `POST /store-api/account/password`: swaps the password after checking the current one and opens a new session.
pub async fn password(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (t, email) = customer(&a, &h).await?;
    let current = accounts::password_hash(&a, &t, &email).await?;
    let input: PasswordChange = view(&v);
    let old = input
        .old_password
        .as_deref()
        .ok_or(bad("Current password required"))?;
    if !verify_password(old.to_string(), current.clone()).await? {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ));
    }
    let new = hash_password(new_password(&json!({"password":input.new_password}))?).await?;
    accounts::replace_password(&a, &t, &email, current, new).await?;
    Ok(Json(json!({"customerToken":open(&a,&t,&email).await?})))
}
