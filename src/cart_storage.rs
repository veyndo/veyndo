//! Cart creation, loading and input validation.
use crate::*;

pub(crate) async fn load_cart(a: &App, h: &RequestContext) -> Result<StoredCart> {
    veyndo_checkout::store::cart_load::load(a, h).await
}
pub(crate) async fn new_cart(
    a: &App,
    t: &str,
    session: &str,
    locale: &str,
    channel: &str,
) -> Result<StoredCart> {
    let (settings, _) = commerce::config(a, t).await?;
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
    let checkout = commerce::CheckoutSelection {
        country,
        shipping_method_id: shipping.id.clone(),
        payment_method_id: payment.id.clone(),
        ..commerce::CheckoutSelection::defaults()
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
    sqlx::query("INSERT INTO carts(id,tenant,token,data) VALUES($1,$2,$3,$4)")
        .bind(&c.id)
        .bind(t)
        .bind(&c.token)
        .bind(json!(c.data))
        .execute(&a.db)
        .await?;
    Ok(c)
}
pub(crate) use veyndo_shop::domain::cart_lines::validate as validate_items;

pub(crate) async fn new_cart_context(
    a: &App,
    h: &RequestContext,
    session: &str,
    transport: &str,
) -> Result<StoredCart> {
    let t = tenant(h)?;
    let locale = language_context(a, h).await?.0;
    let channel = marketing::channel_id(h);
    marketing::channel(a, h, channel, &locale).await?;
    let identity = if header(h, "x-customer-token").is_some() {
        Some(accounts::identity(a, h).await?.1)
    } else {
        None
    };
    let (settings, _) = commerce::scoped_config(a, &t, channel).await?;
    let selected_currency = settings
        .currencies
        .selected(header(h, "x-commerce-currency").unwrap_or(""))?
        .code
        .clone();
    let mut c = new_cart(a, &t, session, &locale, transport).await?;
    c.data.sales_channel = channel.into();
    c.data.currency = selected_currency;
    let (settings, _) = commerce::scoped_config(a, &t, channel).await?;
    c.data.currency = settings.currencies.selected(&c.data.currency)?.code.clone();
    c.data.checkout = Some(commerce::resolve_selection(
        commerce::selection(&c.data),
        &c.data.group,
        &settings,
    ));
    if let Some(email) = identity {
        let r = sqlx::query("SELECT * FROM customers WHERE tenant=$1 AND email=$2")
            .bind(&t)
            .bind(&email)
            .fetch_one(&a.db)
            .await?;
        c.data.email = Some(email.clone());
        c.data.customer_id = Some(r.get("id"));
        c.data.group = r.get("group_name");
        c.data.company = r.get("company");
        if let Some(checkout) = &mut c.data.checkout {
            checkout.customer_email = Some(email.clone());
            checkout.billing_address_id = r.get("default_billing_address_id");
            checkout.shipping_address_id = r.get("default_shipping_address_id");
            let mut conn = a.db.begin().await?;
            if let Some(id) = &checkout.billing_address_id {
                checkout.billing_address =
                    Some(accounts::address_get(&mut conn, &t, &email, id).await?);
            }
            if let Some(id) = &checkout.shipping_address_id {
                checkout.address = Some(accounts::address_get(&mut conn, &t, &email, id).await?);
                checkout.country = checkout.address.as_ref().unwrap().country.clone();
            }
            drop(conn);
            let p: Value = r.get("profile");
            if let Some(id) = p["defaultPaymentMethodId"].as_str() {
                checkout.payment_method_id = id.into();
            }
            let (settings, _) = commerce::scoped_config(a, &t, channel).await?;
            c.data.currency = settings.currencies.selected(&c.data.currency)?.code.clone();
            *checkout = commerce::resolve_selection(checkout.clone(), &c.data.group, &settings);
        }
    }
    sqlx::query("UPDATE carts SET data=$1 WHERE id=$2")
        .bind(json!(c.data))
        .bind(&c.id)
        .execute(&a.db)
        .await?;
    Ok(c)
}

/// The server side of the cart port components use.
pub(crate) struct Carts<'s>(pub(crate) &'s App);
impl veyndo_shop::ports::Carts for Carts<'_> {
    fn load<'a>(&'a self, h: &'a RequestContext) -> veyndo_core::PortFuture<'a, StoredCart> {
        Box::pin(load_cart(self.0, h))
    }
    fn view<'a>(&'a self, cart: &'a StoredCart) -> veyndo_core::PortFuture<'a, Value> {
        Box::pin(cart_json(self.0, cart))
    }
    fn admit<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        h: &'a RequestContext,
        cart: &'a StoredCart,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            pos::guard_cart(h, cart)?;
            b2b::guard_conn(conn, h, cart).await?;
            Ok(())
        })
    }
    fn replace<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: Option<i64>,
    ) -> veyndo_core::PortFuture<'a, StoredCart> {
        Box::pin(set_cart(self.0, h, items, expected, None))
    }
}
