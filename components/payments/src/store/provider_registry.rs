//! Installed payment providers: method registration from app manifests, provider account snapshots, method availability and the pending payment attempt of an order; no remote calls inside checkout SQL.
use crate::domain::capture_intent::intent;
use crate::domain::service_config::validate_service;
use crate::store::jobs::enqueue_tx;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::{Raw, lenient, view};
use veyndo_core::{Result, State, bad, conflict, process_config, uid};
use veyndo_shop::domain::patch::resolve;
use veyndo_shop::domain::validation::validate;
use veyndo_shop::service::read_cache::conditions;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::settings::{Payment, Settings};

type AvailableMethods = HashMap<(String, String), String>;

/// The pinned adapter service of a provider version and environment, from the operator's payment service map.
pub fn config(provider: &str, version: &str, environment: &str) -> Result<Value> {
    let all = &process_config::get().payment_services;
    // Keep environments side by side so switching a channel to live does not strand sandbox receipts.
    let version = &all[provider][version];
    let c = if version[environment].is_object() {
        version[environment].clone()
    } else {
        version.clone()
    };
    validate_service(&c, environment)?;
    Ok(c)
}

/// Adds the methods of an installed or upgraded provider to the shop settings, refusing an upgrade that would narrow an active method.
pub async fn install_methods(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
) -> Result<()> {
    let previous: Option<Value> =
        sqlx::query_scalar("SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(&m.id)
            .fetch_optional(&mut **tx)
            .await?;
    let old = previous
        .map(serde_json::from_value::<Manifest>)
        .transpose()
        .map_err(|_| bad("Invalid installed provider"))?;
    if m.payment_provider.is_none() && old.as_ref().is_none_or(|p| p.payment_provider.is_none()) {
        return Ok(());
    }
    let row = sqlx::query("SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
        .bind(t)
        .fetch_one(&mut **tx)
        .await?;
    let mut s: Settings =
        serde_json::from_value(row.get("data")).map_err(|_| bad("Invalid settings"))?;
    // Basis locking serializes channel overrides. Check the actual resolved settings,
    // including an enabled channel method whose shared-basis counterpart is disabled.
    let patches: Vec<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_overrides WHERE tenant=$1")
            .bind(t)
            .fetch_all(&mut **tx)
            .await?;
    let mut scopes = vec![s.clone()];
    for patch in patches {
        scopes.push(resolve(&s, patch, &conditions)?);
    }
    for scope in scopes {
        for configured in scope
            .payments
            .iter()
            .filter(|p| p.active && p.provider.as_deref() == Some(&m.id))
        {
            let method = m.payment_provider.as_ref().and_then(|p| p.methods.iter()
                .find(|method| Some(&method.id) == configured.provider_method.as_ref()))
                .ok_or(conflict("Upgrade would invalidate an active payment method; update its configuration first"))?;
            intent(configured.capture_mode.as_deref(), method)?;
            let previous = old
                .as_ref()
                .and_then(|p| p.payment_provider.as_ref())
                .and_then(|p| {
                    p.methods
                        .iter()
                        .find(|method| Some(&method.id) == configured.provider_method.as_ref())
                });
            let lost_currency = scope.currencies.enabled.iter().any(|code| {
                previous.is_some_and(|p| p.currencies.contains(code))
                    && !method.currencies.contains(code)
            });
            let lost_country = scope.countries.iter().any(|code| {
                (!configured.restricted_countries || configured.countries.contains(code))
                    && previous
                        .is_some_and(|p| p.countries.is_empty() || p.countries.contains(code))
                    && !method.countries.is_empty()
                    && !method.countries.contains(code)
            });
            if lost_currency || lost_country {
                return Err(conflict(
                    "Upgrade would narrow an active payment method; update its configuration first",
                ));
            }
        }
    }
    let Some(p) = &m.payment_provider else {
        return Ok(());
    };
    for method in &p.methods {
        let id = format!("{}-{}", m.id.replace('_', "-"), method.id.replace('_', "-"));
        if id.len() > 50 {
            return Err(bad("Provider payment method ID exceeds limit"));
        }
        if let Some(old) = s.payments.iter().find(|v| v.id == id) {
            if old.provider.as_deref() != Some(&m.id)
                || old.provider_method.as_deref() != Some(&method.id)
            {
                return Err(conflict("Payment method ID already owned"));
            }
            continue;
        }
        s.payments.push(Payment {
            id,
            name: method
                .name
                .get("en")
                .or_else(|| method.name.values().next())
                .cloned()
                .unwrap_or_default(),
            translations: method
                .name
                .iter()
                .map(|(l, n)| {
                    (
                        l.clone(),
                        serde_json::from_value(json!({"name":n})).unwrap(),
                    )
                })
                .collect(),
            countries: method
                .countries
                .iter()
                .filter(|c| s.countries.contains(c))
                .cloned()
                .collect(),
            restricted_countries: !method.countries.is_empty(),
            active: false,
            business_only: false,
            mode: "app".into(),
            provider: Some(m.id.clone()),
            provider_method: Some(method.id.clone()),
            capture_mode: None,
        });
    }
    validate(&s, &conditions)?;
    sqlx::query("UPDATE commerce_settings SET data=$1,revision=revision+1 WHERE tenant=$2")
        .bind(json!(s))
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// Records the pending provider payment attempt of an order with its reservations and enqueues its creation.
/// The placed order facts a provider payment is prepared from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedOrder {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    cart: PlacedCart,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PlacedCart {
    #[serde(deserialize_with = "lenient")]
    payment_method: Raw<SelectedMethod>,
    #[serde(deserialize_with = "lenient")]
    price: PlacedPrice,
    #[serde(deserialize_with = "lenient")]
    checkout: PlacedCheckout,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SelectedMethod {
    #[serde(deserialize_with = "lenient")]
    provider: Option<String>,
    #[serde(deserialize_with = "lenient")]
    capture_mode: Option<String>,
    #[serde(deserialize_with = "lenient")]
    provider_method: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PlacedPrice {
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    currency_scale: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedCheckout {
    #[serde(deserialize_with = "lenient")]
    country: Option<String>,
}

pub async fn prepare_remote(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    o: &mut Value,
    minor: i64,
    return_origin: Option<&str>,
) -> Result<()> {
    let order: PlacedOrder = view(o);
    let selected = &order.cart.payment_method;
    let provider = selected
        .provider
        .as_deref()
        .ok_or(bad("Provider required"))?;
    let row = sqlx::query(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
    )
    .bind(&c.tenant)
    .bind(provider)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(conflict("Payment app is inactive"))?;
    let m: Manifest =
        serde_json::from_value(row.get("manifest")).map_err(|_| bad("Invalid payment manifest"))?;
    let contract = m
        .payment_provider
        .as_ref()
        .ok_or(bad("Payment provider contract missing"))?;
    let configured_intent = selected.capture_mode.as_deref();
    let method = contract
        .methods
        .iter()
        .find(|v| Some(v.id.as_str()) == selected.provider_method.as_deref())
        .ok_or(bad("Unknown provider method"))?;
    let currency = order
        .cart
        .price
        .currency
        .clone()
        .unwrap_or_else(|| "EUR".into());
    if !method.currencies.iter().any(|c| c == &currency)
        || (!method.countries.is_empty()
            && !method
                .countries
                .iter()
                .any(|v| Some(v.as_str()) == order.cart.checkout.country.as_deref()))
    {
        return Err(bad("Provider method unavailable for currency or country"));
    }
    let intent = intent(configured_intent, method)?;
    let channel = c.data.sales_channel.as_str();
    let r=sqlx::query("SELECT * FROM payment_provider_accounts WHERE tenant=$1 AND app=$2 AND channel IN ($3,'default') ORDER BY (channel=$3) DESC LIMIT 1 FOR SHARE").bind(&c.tenant).bind(provider).bind(channel).fetch_optional(&mut **tx).await?.ok_or(conflict("Complete payment provider onboarding first"))?;
    if !r.get::<bool, _>("ready") {
        return Err(conflict("Payment account is not ready"));
    }
    if !r.get::<Value, _>("data")["methods"]
        .as_array()
        .is_some_and(|ms| ms.iter().any(|v| v == &method.id))
    {
        return Err(conflict(
            "Provider has not enabled this method for the merchant",
        ));
    }
    let environment: String = r.get("environment");
    config(provider, &m.version, &environment)?;
    let context = json!({"apiVersion":"1","accountRef":r.get::<String,_>("account_ref"),"method":method.id,"capabilities":method.capabilities,"checkout":method.checkout,"intent":intent,"channel":channel,"returnOrigin":return_origin,"currencyScale":order.cart.price.currency_scale});
    let id = uid();
    o["payment"] = json!({"method":selected.raw,"provider":provider,"state":"pending","attemptId":id,"environment":environment,"realMoneyCharged":false});
    sqlx::query("INSERT INTO payment_attempts(id,tenant,order_id,provider,adapter_version,amount_minor,currency,environment,bn_code,provider_context,currency_scale) VALUES($1,$2,$3,$4,$5,$6,$9,$7,'',$8,$10)").bind(&id).bind(&c.tenant).bind(order.id.as_deref().ok_or(bad("Order ID required"))?).bind(provider).bind(&m.version).bind(minor).bind(environment).bind(context).bind(currency).bind(order.cart.price.currency_scale.as_i64().unwrap_or(2) as i16).execute(&mut **tx).await?;
    for item in &c.data.items {
        sqlx::query("INSERT INTO inventory_reservations(tenant,attempt_id,product_id,quantity) VALUES($1,$2,$3,$4)").bind(&c.tenant).bind(&id).bind(&item.id).bind(item.quantity as i32).execute(&mut **tx).await?;
    }
    enqueue_tx(
        tx,
        &c.tenant,
        &id,
        "create",
        &format!("{id}:create"),
        &json!({}),
    )
    .await?;
    Ok(())
}

/// One bounded registry query hides disconnected/uninstalled provider methods from storefront discovery.
pub async fn available(
    a: &State,
    t: &str,
    channel: &str,
    currency: &str,
) -> Result<HashSet<(String, String)>> {
    Ok(available_environments(a, t, channel, currency)
        .await?
        .into_keys()
        .collect())
}

async fn available_environments(
    a: &State,
    t: &str,
    channel: &str,
    currency: &str,
) -> Result<AvailableMethods> {
    let rows = sqlx::query("SELECT DISTINCT ON (p.id) p.id,p.version,p.manifest,a.ready,a.data,a.environment FROM app_packages p JOIN payment_provider_accounts a ON a.tenant=p.tenant AND a.app=p.id WHERE p.tenant=$1 AND p.active AND a.channel IN ($2,'default') ORDER BY p.id,(a.channel=$2) DESC").bind(t).bind(channel).fetch_all(&a.db).await?;
    enabled_rows(rows, currency)
}
fn enabled_rows(rows: Vec<sqlx::postgres::PgRow>, currency: &str) -> Result<AvailableMethods> {
    let mut enabled = HashMap::new();
    for row in rows {
        let id: String = row.get("id");
        let environment: String = row.get("environment");
        if !row.get::<bool, _>("ready")
            || config(&id, &row.get::<String, _>("version"), &environment).is_err()
        {
            continue;
        }
        let manifest: Manifest = serde_json::from_value(row.get("manifest"))
            .map_err(|_| bad("Invalid installed provider"))?;
        let account: Value = row.get("data");
        if let Some(provider) = manifest.payment_provider {
            for method in provider.methods {
                if method.currencies.iter().any(|c| c == currency)
                    && account["methods"]
                        .as_array()
                        .is_some_and(|ms| ms.iter().any(|v| v == &method.id))
                {
                    enabled.insert((id.clone(), method.id), environment.clone());
                }
            }
        }
    }
    Ok(enabled)
}

/// Provider declarations constrain quote discovery; authoritative preparation rechecks under locks.
pub async fn currency_methods(
    a: &State,
    t: &str,
    channel: &str,
    code: &str,
    s: Settings,
) -> Result<(Settings, Option<String>)> {
    let mut enabled = available_environments(a, t, channel, code).await?;
    let settings = filter_methods(&enabled, code, s);
    Ok((
        settings,
        enabled.remove(&("shopware_payments".into(), "paypal".into())),
    ))
}
fn filter_methods(enabled: &AvailableMethods, code: &str, mut s: Settings) -> Settings {
    for method in &mut s.payments {
        if let Some(provider) = &method.provider {
            method.active &= method
                .provider_method
                .as_ref()
                .is_some_and(|m| enabled.contains_key(&(provider.clone(), m.clone())));
        }
        if ["paypal-sandbox", "paypal-live"].contains(&method.id.as_str()) {
            method.active &= "AUD BRL CAD CNY CZK DKK EUR HKD HUF ILS JPY MYR MXN TWD NZD NOK PHP PLN GBP SGD SEK CHF THB USD".split(' ').any(|c|c==code);
        }
    }
    s
}

/// Checkout reads the provider registry on its existing transaction: never borrow another pooled backend under locks.
pub async fn currency_methods_conn(
    conn: &mut sqlx::PgConnection,
    t: &str,
    channel: &str,
    code: &str,
    s: Settings,
) -> Result<Settings> {
    let rows=sqlx::query("SELECT DISTINCT ON (p.id) p.id,p.version,p.manifest,a.ready,a.data,a.environment FROM app_packages p JOIN payment_provider_accounts a ON a.tenant=p.tenant AND a.app=p.id WHERE p.tenant=$1 AND p.active AND a.channel IN ($2,'default') ORDER BY p.id,(a.channel=$2) DESC").bind(t).bind(channel).fetch_all(conn).await?;
    Ok(filter_methods(&enabled_rows(rows, code)?, code, s))
}
