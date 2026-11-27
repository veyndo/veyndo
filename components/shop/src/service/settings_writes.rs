//! Settings writes: the shared basis, channel overrides and their selective release, guarded against stranding methods, groups, currencies, tax classes and languages. Cross-component checks come through the settings port.
use crate::Settings;
use crate::domain::defaults::decode;
use crate::domain::patch::{difference, resolve};
use crate::domain::validation::validate;
use crate::ports::{SettingsGuards, SettingsHost};
use crate::service::read_cache::config;
use crate::store::group_usage::guard as group_guard;
use crate::store::settings::{self, scoped_locked};
use crate::store::settings_view;
use crate::types::events::{ChannelConfigured, CommerceConfigured};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::store::history_context::context;
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_pricing::service::currency_change::guard as currency_guard;
use veyndo_shipping::store::method_usage::{Area, usage};

/// Registers every enabled language in the global language table.
async fn register(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, s: &Settings) -> Result<()> {
    for locale in &s.locales {
        settings::register_language(tx, locale).await?;
    }
    Ok(())
}
/// Methods still referenced by orders, carts or rules cannot be removed; capture configuration must stay valid.
async fn guard(
    guards: &dyn SettingsGuards,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    old: &Settings,
    next: &Settings,
    channel: Option<&str>,
) -> Result<()> {
    guards.capture(tx, t, next).await?;
    for (area, previous, current) in [
        (
            Area::Shipping,
            old.shipping
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            next.shipping
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            Area::Payments,
            old.payments
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            next.payments
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
        ),
    ] {
        for id in previous.into_iter().filter(|id| !current.contains(id)) {
            if !usage(guards, tx, t, area, id, channel).await?.can_delete {
                return Err(conflict("Method is referenced; deactivate it instead"));
            }
        }
    }
    Ok(())
}
/// A settings save: the complete configuration and the revisions it was edited at.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SettingsSave {
    data: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    base_revision: Value,
}

/// Saves the shared settings basis at its revision after every dependency guard.
pub async fn save_config(
    Shared(host): Shared<SettingsHost>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    permit(&h, "settings.write")?;
    let a = &host.core;
    let t = merchant(&h)?;
    let input: SettingsSave = view(&v);
    let s = decode(input.data)?;
    validate(&s, &|v| host.guards.condition(v))?;
    let revision = input
        .revision
        .ok_or(bad("Configuration revision required"))?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "merchant").await?;
    host.guards.lock(&mut tx, &t).await?;
    // Serialize configuration with product assignments and translation apply, so
    // a concurrently removed class/language cannot be committed from an older read.
    let (previous, _) = settings::for_update(&mut tx, &t).await?;
    if previous.main_locale != s.main_locale {
        host.guards.locale(&mut tx, &t, &s.main_locale).await?;
    }
    currency_guard(
        &*host.guards,
        &mut tx,
        &t,
        &previous.currencies,
        &s.currencies,
    )
    .await?;
    group_guard(&mut tx, &t, &previous, &s).await?;
    guard(&*host.guards, &mut tx, &t, &previous, &s, None).await?;
    for patch in settings::overrides(&mut tx, &t).await? {
        let scoped = resolve(&s, patch, &|v| host.guards.condition(v))?;
        host.guards.capture(&mut tx, &t, &scoped).await?;
    }
    register(&mut tx, &s).await?;
    let tax_ids = s.taxes.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    if settings::tax_class_stranded(&mut tx, &t, &tax_ids).await? {
        return Err(bad("Tax class is assigned to products"));
    }
    let r = settings::replace_basis_at(&mut tx, &t, &json!(s), revision)
        .await?
        .ok_or(conflict("Configuration changed; reload first"))?;
    veyndo_core::events::emit(&mut *tx, &t, &CommerceConfigured { revision: r }).await?;
    tx.commit().await?;
    Ok(Json(json!({"revision":r})))
}
/// The channel overrides of a sandbox as release units.
pub async fn snapshot_scopes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    result: &mut serde_json::Map<String, Value>,
) -> Result<()> {
    for (channel, data) in settings::overrides_for_update(tx, t).await? {
        result.insert(format!("settings-channel:{channel}"), data);
    }
    Ok(())
}
/// Publishes one channel override from a sandbox release after the method guards.
pub async fn publish_scope(
    guards: &dyn SettingsGuards,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    channel: &str,
    value: &Value,
) -> Result<()> {
    let (old, _) = scoped_locked(tx, t, channel, &|v| guards.condition(v)).await?;
    let base = settings::basis_for_update(tx, t).await?;
    let next = resolve(&decode(base)?, value.clone(), &|v| guards.condition(v))?;
    guard(guards, tx, t, &old, &next, Some(channel)).await?;
    settings::upsert_override(tx, t, channel, value).await?;
    Ok(())
}
/// Checks released shop settings against the live dependencies they must not strand.
pub async fn validate_settings_release(
    guards: &dyn SettingsGuards,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    previous: &Value,
) -> Result<()> {
    let next = decode(settings::basis_for_update(tx, t).await?)?;
    validate(&next, &|v| guards.condition(v))?;
    let previous = decode(previous.clone())?;
    currency_guard(guards, tx, t, &previous.currencies, &next.currencies).await?;
    guard(guards, tx, t, &previous, &next, None).await?;
    let tax_ids = next.taxes.iter().map(|v| v.id.clone()).collect::<Vec<_>>();
    if settings::tax_class_stranded(tx, t, &tax_ids).await? {
        return Err(bad("Tax class is assigned to products"));
    }
    for patch in settings::overrides(tx, t).await? {
        let scoped = resolve(&next, patch, &|v| guards.condition(v))?;
        guards.capture(tx, t, &scoped).await?;
    }
    register(tx, &next).await?;
    Ok(())
}
/// One channel's effective settings with its basis and overrides.
pub async fn get_scope(
    Shared(host): Shared<SettingsHost>,
    h: RequestContext,
    Path(channel): Path<String>,
) -> Result<Json<Value>> {
    permit(&h, "settings.read")?;
    let a = &host.core;
    let t = merchant(&h)?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "merchant").await?;
    let (data, base_revision) =
        scoped_locked(&mut tx, &t, &channel, &|v| host.guards.condition(v)).await?;
    let row = settings::channel_override(&mut tx, &t, &channel).await?;
    let base = decode(settings::basis(&mut tx, &t).await?)?;
    let (patch, revision) = row.map_or((json!([]), 0), |r| (r.data, r.revision));
    tx.commit().await?;
    Ok(Json(
        json!({"data":data,"revision":revision,"baseRevision":base_revision,"inherited":base,"overrides":patch,"channelId":channel}),
    ))
}
/// Saves one channel's overrides at the basis and channel revisions.
pub async fn save_scope(
    Shared(host): Shared<SettingsHost>,
    h: RequestContext,
    Path(channel): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    permit(&h, "settings.write")?;
    let a = &host.core;
    let t = merchant(&h)?;
    if channel == "default" {
        return Err(bad("Default channel uses the shared settings basis"));
    }
    let input: SettingsSave = view(&v);
    let next = decode(input.data)?;
    validate(&next, &|v| host.guards.condition(v))?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "merchant").await?;
    let (base, base_revision) = settings::for_update(&mut tx, &t).await?;
    if input.base_revision.as_i64() != Some(base_revision) {
        return Err(conflict("Settings basis changed; reload first"));
    }
    let (old, _) = scoped_locked(&mut tx, &t, &channel, &|v| host.guards.condition(v)).await?;
    let revision = settings::override_revision(&mut tx, &t, &channel).await?;
    if input.revision != Some(revision.unwrap_or(0)) {
        return Err(conflict("Channel settings changed; reload first"));
    }
    guard(&*host.guards, &mut tx, &t, &old, &next, Some(&channel)).await?;
    // Channel language availability belongs to the existing sales-channel contract.
    if next.main_locale != base.main_locale
        || next.locales != base.locales
        || json!(next.country_definitions) != json!(base.country_definitions)
        || json!(next.customer_groups) != json!(base.customer_groups)
    {
        return Err(bad(
            "Manage languages and country definitions in the shared basis",
        ));
    }
    let patch = difference(&base, &next);
    settings::upsert_override(&mut tx, &t, &channel, &patch).await?;
    let configured = ChannelConfigured {
        channel_id: &channel,
        revision: revision.unwrap_or(0) + 1,
    };
    veyndo_core::events::emit(&mut *tx, &t, &configured).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"data":next,"revision":revision.unwrap_or(0)+1,"baseRevision":input.base_revision,"inherited":base,"overrides":patch,"channelId":channel}),
    ))
}
/// The shop settings with recent reviews and orders for the merchant settings view.
pub async fn merchant_config(
    Shared(host): Shared<SettingsHost>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let a = &host.core;
    let t = merchant(&h)?;
    permit(&h, "settings.read")?;
    let (data, revision) = config(a, &t).await?;
    let rs = settings_view::reviews(
        a,
        &t,
        permit(&h, "catalog.read").is_ok() && permit(&h, "customers.pii").is_ok(),
    )
    .await?;
    let os = if permit(&h, "orders.read").is_ok() && permit(&h, "customers.pii").is_ok() {
        settings_view::orders(&t).await?
    } else {
        Vec::new()
    };
    Ok(Json(
        json!({"data":data,"revision":revision,"reviews":rs.iter().map(|r|json!({"id":r.id,"productId":r.product_id,"author":r.author,"rating":r.rating,"title":r.title,"content":r.content,"approved":r.approved,"verifiedPurchase":r.verified,"demo":r.demo,"sourceImport":r.source_import,"time":r.time})).collect::<Vec<_>>(),"orders":os}),
    ))
}
