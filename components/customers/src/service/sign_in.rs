//! Buyer sign-in: credential verification, binding the open cart to the account and context rotation.
use crate::ports::SignIn;
use crate::service::address_entries::address_get;
use crate::service::sessions;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, uid};
use veyndo_identity::domain::credentials::verify_password;
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_shop::domain::checkout_selection::resolve_selection;
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::{selection, stored};

/// Verifies the buyer's credentials, binds the open cart to the account with its default addresses and payment method, and rotates the cart token; returns the priced cart and a customer session.
pub async fn login(host: &dyn SignIn, a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = tenant(h)?;
    let email = v["email"].as_str().unwrap_or("");
    let password = v["password"].as_str().unwrap_or("");
    let r = sqlx::query("SELECT * FROM customers WHERE tenant=$1 AND email=$2 AND active")
        .bind(&t)
        .bind(email)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ))?;
    if !verify_password(password.to_owned(), r.get("password_hash")).await? {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ));
    }
    let mut tx = a.db.begin().await?;
    let row = sqlx::query("SELECT * FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE")
        .bind(&t)
        .bind(token(h)?)
        .fetch_one(&mut *tx)
        .await?;
    let mut c = stored(&row)?;
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
    c.data.customer_id = Some(r.get("id"));
    let mut selected = selection(&c.data);
    selected.customer_email = Some(email.into());
    selected.billing_address_id = r.get("default_billing_address_id");
    selected.shipping_address_id = r.get("default_shipping_address_id");
    if let Some(id) = &selected.billing_address_id {
        selected.billing_address = Some(address_get(&mut tx, &t, email, id).await?);
    }
    if let Some(id) = &selected.shipping_address_id {
        selected.address = Some(address_get(&mut tx, &t, email, id).await?);
        selected.country = selected.address.as_ref().unwrap().country.clone();
    }
    let profile: Value = r.get("profile");
    if let Some(id) = profile["defaultPaymentMethodId"].as_str() {
        selected.payment_method_id = id.into();
    }
    let (settings, _) = scoped_locked(&mut tx, &t, "default", &validate_condition).await?;
    c.data.checkout = Some(resolve_selection(
        selected,
        &r.get::<String, _>("group_name"),
        &settings,
    ));
    c.data.group = r.get("group_name");
    c.data.company = r.get("company");
    let new_token = uid(); // Rotate the context after authentication.
    sqlx::query("UPDATE carts SET data=$1,token=$2,revision=revision+1 WHERE id=$3")
        .bind(json!(c.data))
        .bind(&new_token)
        .bind(&c.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE customers SET first_login=coalesce(first_login,now()),last_login=now() WHERE tenant=$1 AND email=$2").bind(&t).bind(email).execute(&mut *tx).await?;
    tx.commit().await?;
    c.token = new_token;
    c.revision += 1;
    let mut result = host.view(&c).await?;
    result["customerToken"] = json!(sessions::open(a, &t, email).await?);
    Ok(result)
}
