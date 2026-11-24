//! Settings writes: the shared basis, channel overrides and their selective release, guarded against stranding methods, groups, currencies, tax classes and languages. Cross-component checks come through the settings port.
use crate::Settings;
use crate::domain::defaults::decode;
use crate::domain::patch::{difference, resolve};
use crate::domain::validation::validate;
use crate::ports::{SettingsGuards, SettingsHost};
use crate::service::read_cache::config;
use crate::store::group_usage::guard as group_guard;
use crate::store::settings::scoped_locked;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::history_context::context;
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_pricing::service::currency_change::guard as currency_guard;
use veyndo_shipping::store::method_usage::{Area, usage};

/// Registers every enabled language in the global language table.
async fn register(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, s: &Settings) -> Result<()> {
    for locale in &s.locales {
        sqlx::query("SELECT public.veyndo_register_language($1)")
            .bind(locale)
            .execute(&mut **tx)
            .await?;
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
            if !usage(tx, t, area, id, channel).await?.can_delete {
                return Err(conflict("Method is referenced; deactivate it instead"));
            }
        }
    }
    Ok(())
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
    let s = decode(v["data"].clone())?;
    validate(&s, &|v| host.guards.condition(v))?;
    let revision = v["revision"]
        .as_i64()
        .ok_or(bad("Configuration revision required"))?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "merchant").await?;
    host.guards.lock(&mut tx, &t).await?;
    // Serialize configuration with product assignments and translation apply, so
    // a concurrently removed class/language cannot be committed from an older read.
    let previous =
        sqlx::query("SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
    let previous = decode(previous.get("data"))?;
    if previous.main_locale != s.main_locale {
        host.guards.locale(&mut tx, &t, &s.main_locale).await?;
    }
    currency_guard(&mut tx, &t, &previous.currencies, &s.currencies).await?;
    group_guard(&mut tx, &t, &previous, &s).await?;
    guard(&*host.guards, &mut tx, &t, &previous, &s, None).await?;
    let patches: Vec<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_overrides WHERE tenant=$1")
            .bind(&t)
            .fetch_all(&mut *tx)
            .await?;
    for patch in patches {
        let scoped = resolve(&s, patch, &|v| host.guards.condition(v))?;
        host.guards.capture(&mut tx, &t, &scoped).await?;
    }
    register(&mut tx, &s).await?;
    let tax_ids = s.taxes.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    let stranded:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND extra->>'taxClassId' IS NOT NULL AND NOT(extra->>'taxClassId'=ANY($2)))").bind(&t).bind(tax_ids).fetch_one(&mut *tx).await?;
    if stranded {
        return Err(bad("Tax class is assigned to products"));
    }
    let r=sqlx::query("UPDATE commerce_settings SET data=$1,revision=revision+1 WHERE tenant=$2 AND revision=$3 RETURNING revision").bind(json!(s)).bind(&t).bind(revision).fetch_optional(&mut *tx).await?.ok_or(conflict("Configuration changed; reload first"))?;
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &t,
        "commerce.configured",
        &json!({"revision":r.get::<i64,_>("revision")}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"revision":r.get::<i64,_>("revision")})))
}
/// The channel overrides of a sandbox as release units.
pub async fn snapshot_scopes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    result: &mut serde_json::Map<String, Value>,
) -> Result<()> {
    for row in sqlx::query("SELECT channel_id,data FROM commerce_overrides WHERE tenant=$1 ORDER BY channel_id FOR UPDATE").bind(t).fetch_all(&mut **tx).await? {
  result.insert(format!("settings-channel:{}",row.get::<String,_>("channel_id")),row.get("data"));
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
    let base: Value =
        sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
            .bind(t)
            .fetch_one(&mut **tx)
            .await?;
    let next = resolve(&decode(base)?, value.clone(), &|v| guards.condition(v))?;
    guard(guards, tx, t, &old, &next, Some(channel)).await?;
    sqlx::query("INSERT INTO commerce_overrides(tenant,channel_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,channel_id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_overrides.revision+1").bind(t).bind(channel).bind(value).execute(&mut **tx).await?;
    Ok(())
}
/// Checks released shop settings against the live dependencies they must not strand.
pub async fn validate_settings_release(
    guards: &dyn SettingsGuards,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    previous: &Value,
) -> Result<()> {
    let value: Value =
        sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
            .bind(t)
            .fetch_one(&mut **tx)
            .await?;
    let next = decode(value)?;
    validate(&next, &|v| guards.condition(v))?;
    let previous = decode(previous.clone())?;
    currency_guard(tx, t, &previous.currencies, &next.currencies).await?;
    guard(guards, tx, t, &previous, &next, None).await?;
    let tax_ids = next.taxes.iter().map(|v| v.id.clone()).collect::<Vec<_>>();
    let stranded:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND extra->>'taxClassId' IS NOT NULL AND NOT(extra->>'taxClassId'=ANY($2)))").bind(t).bind(tax_ids).fetch_one(&mut **tx).await?;
    if stranded {
        return Err(bad("Tax class is assigned to products"));
    }
    let patches: Vec<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_overrides WHERE tenant=$1")
            .bind(t)
            .fetch_all(&mut **tx)
            .await?;
    for patch in patches {
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
    let row = sqlx::query(
        "SELECT data,revision FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2",
    )
    .bind(&t)
    .bind(&channel)
    .fetch_optional(&mut *tx)
    .await?;
    let base = decode(
        sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?,
    )?;
    let patch = row
        .as_ref()
        .map(|r| r.get::<Value, _>("data"))
        .unwrap_or(json!([]));
    let revision = row.map(|r| r.get::<i64, _>("revision")).unwrap_or(0);
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
    let next = decode(v["data"].clone())?;
    validate(&next, &|v| host.guards.condition(v))?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "merchant").await?;
    let row = sqlx::query("SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
        .bind(&t)
        .fetch_one(&mut *tx)
        .await?;
    let base = decode(row.get("data"))?;
    if v["baseRevision"].as_i64() != Some(row.get("revision")) {
        return Err(conflict("Settings basis changed; reload first"));
    }
    let (old, _) = scoped_locked(&mut tx, &t, &channel, &|v| host.guards.condition(v)).await?;
    let revision: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2",
    )
    .bind(&t)
    .bind(&channel)
    .fetch_optional(&mut *tx)
    .await?;
    if v["revision"].as_i64() != Some(revision.unwrap_or(0)) {
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
    sqlx::query("INSERT INTO commerce_overrides(tenant,channel_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,channel_id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_overrides.revision+1").bind(&t).bind(&channel).bind(&patch).execute(&mut *tx).await?;
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &t,
        "commerce.configured",
        &json!({"channelId":channel,"revision":revision.unwrap_or(0)+1}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"data":next,"revision":revision.unwrap_or(0)+1,"baseRevision":v["baseRevision"],"inherited":base,"overrides":patch,"channelId":channel}),
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
    let rs=sqlx::query("SELECT id,product_id,author,rating,title,content,verified,approved,demo,source_import,created_at::text AS time FROM product_reviews WHERE tenant=$1 AND (source_import IS NULL OR $2) ORDER BY created_at DESC LIMIT 100").bind(&t).bind(permit(&h, "catalog.read").is_ok() && permit(&h, "customers.pii").is_ok()).fetch_all(&a.db).await?;
    let os = if permit(&h, "orders.read").is_ok() && permit(&h, "customers.pii").is_ok() {
        sqlx::query("SELECT data #- '{cart,token}' AS data FROM orders WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30")
            .bind(&t)
            .fetch_all(&a.db)
            .await?
    } else {
        Vec::new()
    };
    Ok(Json(
        json!({"data":data,"revision":revision,"reviews":rs.iter().map(|r|json!({"id":r.get::<String,_>("id"),"productId":r.get::<String,_>("product_id"),"author":r.get::<String,_>("author"),"rating":r.get::<i32,_>("rating"),"title":r.get::<String,_>("title"),"content":r.get::<String,_>("content"),"approved":r.get::<bool,_>("approved"),"verifiedPurchase":r.get::<bool,_>("verified"),"demo":r.get::<bool,_>("demo"),"sourceImport":r.get::<Option<Value>,_>("source_import"),"time":r.get::<String,_>("time")})).collect::<Vec<_>>(),"orders":os.iter().map(|r|r.get::<Value,_>("data")).collect::<Vec<_>>()}),
    ))
}
