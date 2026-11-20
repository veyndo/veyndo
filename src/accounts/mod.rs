//! Independent customer sessions, profile/password management and owning-account order history.
use crate::*;
mod payment_preference;
pub(crate) use payment_preference::remember_payment;
mod address_restore;
pub(crate) mod browser;
pub(crate) use address_restore::restore_book;
mod address_store;
mod addresses;
mod contacts;
mod metadata;
pub(crate) use metadata::decorate;
mod demo;
mod order_snapshot;
mod orders;
mod profile;
pub(crate) use address_store::*;
pub(crate) use contacts::*;
pub(crate) use demo::seed_address as seed_demo_address;
pub(crate) use order_snapshot::snapshot as order_snapshot;
pub(crate) use veyndo_orders::store::account_orders::ACCOUNT_SCOPE as ORDER_ACCOUNT_SCOPE;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/account/register", post(register))
        .route(
            "/store-api/account/addresses",
            get(addresses::list).post(addresses::create),
        )
        .route(
            "/store-api/account/addresses/{id}",
            axum::routing::put(addresses::save).delete(addresses::remove),
        )
        .route(
            "/store-api/account/profile",
            get(profile::get).put(profile::save),
        )
        .route("/store-api/account/orders", get(orders::list))
        .route("/store-api/account/orders/{id}", get(orders::detail))
        .route(
            "/store-api/account/orders/{order}/receipts/{id}/pdf",
            get(orders::receipt),
        )
        .route("/store-api/account/logout", post(logout))
        .route("/store-api/account/password", post(profile::password))
}
pub(crate) async fn session(a: &App, t: &str, email: &str) -> Result<String> {
    veyndo_customers::service::sessions::open(&a.core, t, email).await
}
pub(crate) use veyndo_core::shop_scope::session_digest;

pub(crate) async fn identity(a: &App, h: &RequestContext) -> Result<(String, String)> {
    let t = tenant(h)?;
    let digest = session_digest(h)?;
    let email = veyndo_customers::service::sessions::identify(&a.core, &t, &digest).await?;
    Ok((t, email))
}
async fn register(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = tenant(&h)?;
    let email = auth::email(&v)?;
    let name = auth::name(&v)?;
    let password = auth::hash_password(auth::password(&v)?).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants WHERE id=$1)")
        .bind(&t)
        .fetch_one(&a.db)
        .await?;
    if !exists {
        return Err(bad("Unknown shop"));
    }
    let mut contact:Contact=serde_json::from_value(json!({"name":name,"firstName":v["firstName"].as_str().unwrap_or(""),"lastName":v["lastName"].as_str().unwrap_or(""),"company":v["company"].as_str().unwrap_or(""),"phoneNumber":v["phoneNumber"].as_str().unwrap_or("")})).map_err(|_|bad("Invalid customer contact"))?;
    contact.validate()?;
    let mut initial = Vec::new();
    let (settings, _) = commerce::config(&a, &t).await?;
    for field in ["billingAddress", "shippingAddress"] {
        if !v[field].is_null() {
            let mut ad: commerce::Address = serde_json::from_value(v[field].clone())
                .map_err(|_| bad("Invalid registration address"))?;
            ad.validate()?;
            commerce::validate_address_geography(&ad, &settings)?;
            if !settings.countries.contains(&ad.country) {
                return Err(bad("Address country unavailable"));
            }
            initial.push(veyndo_customers::store::accounts::Initial {
                address: ad,
                default_billing: field == "billingAddress",
                default_shipping: field == "shippingAddress",
            });
        }
    }
    let locale = language_context(&a, &h).await?.0;
    let channel = marketing::channel_id(&h);
    marketing::channel(&a, &h, channel, &locale).await?;
    veyndo_customers::store::accounts::register(
        &a, &t, &email, password, &contact, initial, &locale, channel,
    )
    .await?;
    Ok(Json(
        json!({"customerToken":session(&a,&t,&email).await?,"email":email}),
    ))
}
async fn logout(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    if header(&h, "sw-context-token").is_some() {
        load_cart(&a, &h).await?;
    }
    veyndo_customers::service::sessions::close(
        &a.core,
        &t,
        header(&h, "x-customer-token").unwrap(),
    )
    .await?;
    if let Some(token) = header(&h, "sw-context-token") {
        sqlx::query("UPDATE carts SET data=jsonb_set(jsonb_set(jsonb_set(jsonb_set(jsonb_set(data,'{email}','null'),'{customer_id}','null'),'{group}','\"consumer\"'),'{company}','null'),'{b2b}','null'),token=$1,revision=revision+1 WHERE tenant=$2 AND token=$3 AND status='open' AND data->>'email'=$4").bind(uid()).bind(&t).bind(token).bind(&email).execute(&a.db).await?;
    }
    Ok(Json(json!({"loggedOut":true})))
}

pub(crate) use orders::project as project_order;
