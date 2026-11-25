//! A new buyer cart: inserted with the shop defaults, then bound to the request's language, channel, currency and signed-in customer.
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::view;
use veyndo_core::shop_scope::{channel as channel_id, header, session_digest, tenant};
use veyndo_core::{RequestContext, Result, State, bad, uid};
use veyndo_customers::service::{address_book, sessions};
use veyndo_marketing::service::channels;
use veyndo_shop::domain::checkout_selection::resolve_selection;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings as scoped;
use veyndo_shop::types::cart::{Cart, StoredCart, selection};
use veyndo_shop::types::settings::CheckoutSelection;

/// The customer profile default a new cart starts from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProfileDefaults {
    default_payment_method_id: Value,
}

/// Inserts an empty open cart with the shop's default delivery country, shipping and consumer payment.
pub async fn new_cart(
    a: &State,
    t: &str,
    session: &str,
    locale: &str,
    channel: &str,
) -> Result<StoredCart> {
    let (settings, _) = scoped(a, t, "default").await?;
    let country = if settings.countries.contains(&"DE".into()) {
        "DE".into()
    } else {
        settings
            .countries
            .first()
            .cloned()
            .ok_or(bad("No delivery country configured"))?
    };
    let shipping = settings
        .shipping
        .iter()
        .filter(|v| v.active && v.countries.contains(&country))
        .min_by_key(|v| v.id != "pickup")
        .ok_or(bad("No shipping method configured"))?;
    let payment = settings
        .payments
        .iter()
        .filter(|v| v.active && !v.business_only)
        .min_by_key(|v| v.id != "demo-card")
        .ok_or(bad("No consumer payment method configured"))?;
    let checkout = CheckoutSelection {
        country,
        shipping_method_id: shipping.id.clone(),
        payment_method_id: payment.id.clone(),
        ..CheckoutSelection::defaults()
    };
    let c = StoredCart {
        id: uid(),
        tenant: t.into(),
        token: uid(),
        data: Cart {
            admin_actor: None,
            coupons: vec![],
            pos_actor: None,
            pos_discount: None,
            b2b: None,
            currency: String::new(),
            sales_channel: "default".into(),
            app_configurations: HashMap::new(),
            items: vec![],
            group: "consumer".into(),
            email: None,
            customer_id: None,
            company: None,
            session: session.chars().take(128).collect(),
            buyer: None,
            order: None,
            locale: locale.into(),
            channel: channel.into(),
            checkout: Some(checkout),
        },
        revision: 1,
        status: "open".into(),
    };
    sqlx::query!(
        "INSERT INTO carts(id,tenant,token,data) VALUES($1,$2,$3,$4)",
        c.id,
        t,
        c.token,
        json!(c.data)
    )
    .execute(&a.db)
    .await?;
    Ok(c)
}
/// A new cart of the request: its language, sales channel and currency, prefilled for a signed-in customer.
pub async fn new_cart_context(
    a: &State,
    h: &RequestContext,
    session: &str,
    transport: &str,
) -> Result<StoredCart> {
    let t = tenant(h)?;
    let locale = language_context(a, h).await?.0;
    let channel = channel_id(h);
    channels::channel(a, h, channel, &locale).await?;
    let identity = if header(h, "x-customer-token").is_some() {
        Some(sessions::identify(a, &t, &session_digest(h)?).await?)
    } else {
        None
    };
    let (settings, _) = scoped(a, &t, channel).await?;
    let selected_currency = settings
        .currencies
        .selected(header(h, "x-commerce-currency").unwrap_or(""))?
        .code
        .clone();
    let mut c = new_cart(a, &t, session, &locale, transport).await?;
    c.data.sales_channel = channel.into();
    c.data.currency = selected_currency;
    let (settings, _) = scoped(a, &t, channel).await?;
    c.data.currency = settings.currencies.selected(&c.data.currency)?.code.clone();
    c.data.checkout = Some(resolve_selection(
        selection(&c.data),
        &c.data.group,
        &settings,
    ));
    if let Some(email) = identity {
        let r = sqlx::query!("SELECT id,group_name,company,default_billing_address_id,default_shipping_address_id,profile FROM customers WHERE tenant=$1 AND email=$2", t, email)
            .fetch_one(&a.db)
            .await?;
        c.data.email = Some(email.clone());
        c.data.customer_id = Some(r.id);
        c.data.group = r.group_name;
        c.data.company = r.company;
        if let Some(checkout) = &mut c.data.checkout {
            checkout.customer_email = Some(email.clone());
            checkout.billing_address_id = r.default_billing_address_id;
            checkout.shipping_address_id = r.default_shipping_address_id;
            let mut conn = a.db.begin().await?;
            if let Some(id) = &checkout.billing_address_id {
                checkout.billing_address =
                    Some(address_book::get(&mut conn, &t, &email, id).await?);
            }
            if let Some(id) = &checkout.shipping_address_id {
                checkout.address = Some(address_book::get(&mut conn, &t, &email, id).await?);
                checkout.country = checkout.address.as_ref().unwrap().country.clone();
            }
            drop(conn);
            let p: Value = r.profile;
            if let Some(id) = view::<ProfileDefaults>(&p)
                .default_payment_method_id
                .as_str()
            {
                checkout.payment_method_id = id.into();
            }
            let (settings, _) = scoped(a, &t, channel).await?;
            c.data.currency = settings.currencies.selected(&c.data.currency)?.code.clone();
            *checkout = resolve_selection(checkout.clone(), &c.data.group, &settings);
        }
    }
    sqlx::query!("UPDATE carts SET data=$1 WHERE id=$2", json!(c.data), c.id)
        .execute(&a.db)
        .await?;
    Ok(c)
}
