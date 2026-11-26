//! Buyer sign-in: credential verification, binding the open cart to the account and context rotation.
use crate::ports::{SignIn, purchases};
use crate::service::address_entries::address_get;
use crate::service::sessions;
use crate::store::sign_in as store;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, uid};
use veyndo_identity::domain::credentials::verify_password;
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_shop::domain::checkout_selection::resolve_selection;
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::selection;

/// The JSON fields `login` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LoginInput {
    #[serde(deserialize_with = "lenient")]
    email: Option<String>,
    #[serde(deserialize_with = "lenient")]
    password: Option<String>,
}

/// The stored profile fields a sign-in applies to the cart.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProfileDefaults {
    #[serde(deserialize_with = "lenient")]
    default_payment_method_id: Option<String>,
}

/// Verifies the buyer's credentials, binds the open cart to the account with its default addresses and payment method, and rotates the cart token; returns the priced cart and a customer session.
pub async fn login(host: &dyn SignIn, a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: LoginInput = view(v);
    let t = tenant(h)?;
    let email = input.email.as_deref().unwrap_or("");
    let password = input.password.as_deref().unwrap_or("");
    let r = store::active_account(a, &t, email).await?.ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Invalid credentials".into(),
    ))?;
    if !verify_password(password.to_owned(), r.password_hash.clone()).await? {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ));
    }
    let mut tx = a.db.begin().await?;
    let mut c = purchases()?.lock_cart(&mut tx, &t, token(h)?).await?;
    host.cashier(h, &c)?;
    if c.data.b2b.is_some() {
        return Err(bad(
            "Sign out before changing the employee of a company cart",
        ));
    }
    if c.status != "open" {
        return Err(conflict("Cart is terminal"));
    }
    c.data.email = Some(email.into());
    c.data.customer_id = Some(r.id.clone());
    let mut selected = selection(&c.data);
    selected.customer_email = Some(email.into());
    selected.billing_address_id = r.default_billing_address_id.clone();
    selected.shipping_address_id = r.default_shipping_address_id.clone();
    if let Some(id) = &selected.billing_address_id {
        selected.billing_address = Some(address_get(&mut tx, &t, email, id).await?);
    }
    if let Some(id) = &selected.shipping_address_id {
        selected.address = Some(address_get(&mut tx, &t, email, id).await?);
        selected.country = selected.address.as_ref().unwrap().country.clone();
    }
    let profile: ProfileDefaults = view(&r.profile);
    if let Some(id) = profile.default_payment_method_id {
        selected.payment_method_id = id;
    }
    let (settings, _) = scoped_locked(&mut tx, &t, "default", &validate_condition).await?;
    c.data.checkout = Some(resolve_selection(selected, &r.group_name, &settings));
    c.data.group = r.group_name;
    c.data.company = r.company;
    let new_token = uid(); // Rotate the context after authentication.
    purchases()?
        .bind_cart(&mut tx, &c.id, &json!(c.data), &new_token)
        .await?;
    store::record_login(&mut tx, &t, email).await?;
    tx.commit().await?;
    c.token = new_token;
    c.revision += 1;
    let mut result = host.view(&c).await?;
    merge(
        &mut result,
        json!({"customerToken": sessions::open(a, &t, email).await?}),
    )?;
    Ok(result)
}
