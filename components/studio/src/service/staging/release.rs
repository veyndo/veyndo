//! Selected units publish in one transaction with staged digests and live baseline conflict checks.
use super::snapshot::{Tx, snapshot};
use super::{Baseline, live, owned};
use super::{final_config, products};
use crate::domain::staging_access as access;
use crate::ports::ReleaseHost;
use crate::store::{
    staging_assets as assets, staging_categories as categories, staging_documents as documents,
};
use crate::store::{staging_clone as clone, staging_release as db};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::domain::consent::actor_permissions;
use veyndo_app_platform::domain::core_extensions::configuration;
use veyndo_app_platform::domain::entity_access::entity_access;
use veyndo_app_platform::store::records::{get as get_record, lock_key};
use veyndo_b2b::store::staging as b2b;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::shop_scope::header;
use veyndo_core::store::history_context::context;
use veyndo_core::{RequestContext, Result, bad, conflict, hash, uid};
use veyndo_identity::permit;
use veyndo_marketing::types::promotion::preserve_import;
use veyndo_orders::domain::order_machine::{OrderMachine, OrderMachineValidation};
use veyndo_orders::store::company::{
    lock as lock_company, validate_release as validate_company_release,
};
use veyndo_orders::store::company_release as company;
use veyndo_shop::service::settings_writes::{publish_scope, validate_settings_release};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    key: String,
    digest: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    approve: bool,
    selections: Vec<Selection>,
}
/// A channel unit's operational kind.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ChannelUnit {
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
}
/// An app package unit.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AppUnit {
    manifest: Value,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
}
/// A record's revision.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Revisioned {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}
/// A rule unit.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RuleUnit {
    name: Value,
    condition: Value,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
}
/// Takes every lock of a release in the order native writers take them.
async fn lock_units<H: ReleaseHost>(
    host: &H,
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    keys: &[String],
) -> Result<()> {
    host.settings_guards().lock(tx, t).await?;
    lock_company(tx, t).await?;
    lock_company(tx, id).await?;
    clone::lock_environments(tx, t).await?;
    db::lock_configuration(tx, t).await?;
    // The package fence precedes record locks, exactly as in native app writes.
    // Otherwise a concurrent save could hold package SHARE while release holds its record.
    let mut package_locks = std::collections::BTreeSet::new();
    for key in keys {
        let app = if let Some(app) = key.strip_prefix("app:") {
            Some(app)
        } else {
            key.strip_prefix("appdata:")
                .and_then(|tail| tail.split(':').next())
        };
        if let Some(app) = app {
            package_locks.insert(format!("{t}:{app}"));
            package_locks.insert(format!("{id}:{app}"));
        }
    }
    for lock in package_locks {
        db::lock_package(tx, &lock).await?;
    }
    for key in keys.iter().filter(|k| k.starts_with("appdata:")) {
        let parts = key.splitn(4, ':').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(bad("Invalid app data selection"));
        }
        db::lock_record(tx, &lock_key(t, parts[1], parts[2], parts[3])).await?;
    }
    Ok(())
}
/// A selection must still be the reviewed staged value on an unchanged live baseline.
fn admit_selections(
    selections: &[Selection],
    keys: &[String],
    current: &Value,
    actual: &Value,
    input: &Baseline,
) -> Result<()> {
    for s in selections {
        let value = current.get(&s.key).ok_or(bad("Unknown change"))?;
        if hash(&value.to_string()) != s.digest || input.stage.get(&s.key) == Some(value) {
            return Err(conflict("Staged change changed or was already published"));
        }
        if actual.get(&s.key) != input.live.get(&s.key) {
            return Err(conflict("Live content changed; resolve before release"));
        }
    }
    // Validate against the current live operational type before it can be overwritten.
    // The complete aggregate (including jointly selected categories/settings) is checked below.
    for key in keys.iter().filter(|k| k.starts_with("channel:")) {
        let old = actual.get(key).and_then(|v| view::<ChannelUnit>(v).kind);
        let new = current.get(key).and_then(|v| view::<ChannelUnit>(v).kind);
        let (old, new) = (old.as_deref(), new.as_deref());
        if old.is_some_and(|old| {
            (old == "pos") != (new == Some("pos"))
                || (old == "marketplace") != (new == Some("marketplace"))
        }) {
            return Err(conflict(
                "Operational channel type cannot be changed; create a new channel",
            ));
        }
    }
    Ok(())
}
/// Publishes one selected unit through its native writer.
async fn publish_unit<H: ReleaseHost>(
    host: &H,
    tx: &mut Tx<'_>,
    h: &RequestContext,
    (t, id): (&str, &str),
    (key, value): (&str, &Value),
    actual: &Value,
) -> Result<()> {
    if let Some(channel) = key.strip_prefix("settings-channel:") {
        permit(h, "settings.write")?;
        publish_scope(host.settings_guards(), tx, t, channel, value).await?;
    } else if key == "company" || key.starts_with("company-channel:") {
        company::publish(tx, t, id, key, value).await?;
    } else if key.starts_with("b2b-company:") || key.starts_with("b2b-unit:") {
        // Policy/catalog units publish below through their shared native writers.
    } else if key.starts_with("category:") {
        // Category units were published above in parent-first dependency order.
    } else if key == "order-workflow" {
        permit(h, "settings.write")?;
        let m: OrderMachine =
            serde_json::from_value(value.clone()).map_err(|_| bad("Invalid workflow"))?;
        m.validate()?;
        let ids = m.states.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        let stranded = db::stranded_orders(tx, t, ids).await?;
        if stranded {
            return Err(conflict("Workflow removes an active order state"));
        }
        crate::store::order_workflow::save(tx, t, value).await?;
    } else if let Some(asset) = key.strip_prefix("asset:") {
        assets::publish_asset(tx, t, id, asset).await?;
    } else if let Some(document) = key.strip_prefix("document:") {
        documents::publish(tx, t, id, document, value).await?;
    } else if key.starts_with("product:") {
        // Product parents and content were published above, before dependent assets.
    } else if let Some(app) = key.strip_prefix("app:") {
        let unit: AppUnit = view(value);
        let m: Manifest =
            serde_json::from_value(unit.manifest).map_err(|_| bad("Invalid app package"))?;
        if m.runtime != "declarative" {
            return Err(bad("External services need a separate deployment"));
        }
        actor_permissions(h, &m)?;
        host.install_app(tx, t, m).await?;
        db::set_app_active(tx, t, app, unit.active).await?;
    } else if key.starts_with("appdata:") {
        let parts = key.splitn(4, ':').collect::<Vec<_>>();
        let manifest = db::manifest(tx, t, parts[1])
            .await?
            .ok_or(bad("Publish the app package before its data"))?;
        let m: Manifest = serde_json::from_value(manifest).map_err(|_| bad("Invalid app"))?;
        let e = m
            .entities
            .iter()
            .find(|e| e.name == parts[2])
            .ok_or(bad("App entity missing"))?;
        if !configuration(e) {
            return Err(bad(
                "Operational customer/order extension records are never release units",
            ));
        }
        let revision = get_record(tx, t, &m.id, &e.name, parts[3], false)
            .await?
            .and_then(|r| view::<Revisioned>(&r).revision)
            .unwrap_or(0);
        let mut fields = value.clone();
        fields.as_object_mut().unwrap().remove("id");
        entity_access(&m, e, h, "save")?;
        host.save_record(
            tx,
            t,
            &m,
            e,
            &json!({"id":parts[3],"revision":revision,"fields":fields}),
        )
        .await?;
    } else if let Some((kind, unit)) = key.split_once(':') {
        let table = match kind {
            "rule" => "commerce_rules",
            "promotion" => "commerce_promotions",
            "flow" => "commerce_flows",
            "channel" => "sales_channels",
            _ => return Err(bad("Unsupported release unit")),
        };
        if kind == "promotion" {
            preserve_import(&actual[key], value)?;
        }
        if kind == "rule" {
            let rule: RuleUnit = view(value);
            db::upsert_rule(tx, t, unit, &rule.name, &rule.condition, rule.active).await?;
        } else {
            let data = if kind == "flow" {
                let mut flow = value.clone();
                merge(
                    &mut flow,
                    json!({ "actor": header(h, "x-rac-user").unwrap_or("unknown") }),
                )?;
                flow
            } else {
                value.clone()
            };
            db::upsert_unit(tx, table, t, unit, &data).await?;
        }
    } else {
        let table = match key {
            "settings" => "commerce_settings",
            "experience" => "experiences",
            _ => return Err(bad("Unsupported release unit")),
        };
        db::replace_document(tx, table, t, value).await?;
    }
    Ok(())
}
/// What the live shop holds for a unit after its publication.
async fn live_value(tx: &mut Tx<'_>, t: &str, key: &str, value: &Value) -> Result<Value> {
    Ok(if let Some(product) = key.strip_prefix("product:") {
        super::snapshot::product_snapshot(tx, t, product).await?
    } else if key == "company" || key.starts_with("company-channel:") {
        company::live_value(tx, t, key).await?
    } else if let Some(flow) = key.strip_prefix("flow:") {
        db::flow(tx, t, flow).await?
    } else {
        value.clone()
    })
}
/// Publishes the selected, approved sandbox units to the live shop in one transaction.
pub async fn release<H: ReleaseHost>(
    host: &H,
    h: RequestContext,
    id: String,
    v: Value,
) -> Result<Value> {
    let a = host.core();
    permit(&h, "environments.manage")?;
    let t = live(a, &h).await?;
    owned(a, &t, &id).await?;
    let req: Release = serde_json::from_value(v).map_err(|_| bad("Invalid release"))?;
    if !req.approve || req.selections.is_empty() || req.selections.len() > 50 {
        return Err(bad("Select 1..50 reviewed changes and approve"));
    }
    let mut keys = req
        .selections
        .iter()
        .map(|s| s.key.clone())
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    if keys.len() != req.selections.len() {
        return Err(bad("Duplicate selection"));
    }
    access::permit(&h, &keys)?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "staging.release").await?;
    lock_units(host, &mut tx, &t, &id, &keys).await?;
    let mut base: Baseline = view(&db::baseline(&mut tx, &id, &t).await?);
    let current = snapshot(&mut tx, &id).await?;
    let actual = snapshot(&mut tx, &t).await?;
    for key in keys.iter().filter(|k| k.starts_with("flow:")) {
        host.authorize_flow(&h, &current[key])?;
    }
    admit_selections(&req.selections, &keys, &current, &actual, &base)?;
    categories::publish(&mut tx, &t, &current, &keys).await?;
    products::publish(&mut tx, &t, &current, &keys).await?;
    // App records validate current native references (languages, channels, packages).
    // Apply all selected configuration before its dependent records in this same
    // transaction. Missing dependencies still fail and roll back the whole release.
    let configuration = keys.iter().filter(|key| !key.starts_with("appdata:"));
    let records = keys.iter().filter(|key| key.starts_with("appdata:"));
    for key in configuration.chain(records) {
        let s = req.selections.iter().find(|s| s.key == *key).unwrap();
        let value = &current[&s.key];
        publish_unit(host, &mut tx, &h, (&t, &id), (&s.key, value), &actual).await?;
        let live = live_value(&mut tx, &t, &s.key, value).await?;
        base.record(&s.key, Some(value.clone()), Some(live))?;
    }
    assets::validate_product_media(&mut tx, &t, &keys).await?;
    b2b::publish_configuration(&mut tx, &t, &keys, &current).await?;
    final_config::validate(host, &mut tx, &t, &h, &keys, (&current, &actual), &mut base).await?;
    let release = uid();
    let selected = json!(keys);
    clone::set_baseline(&mut tx, &id, &json!(base)).await?;
    db::insert_release(
        &mut tx,
        &release,
        &t,
        &id,
        &selected,
        header(&h, "x-rac-user").unwrap_or("unknown"),
    )
    .await?;
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        &t,
        "shop.release.published",
        &json!({"releaseId":release,"selections":selected}),
    )
    .await?;
    validate_settings_release(
        host.settings_guards(),
        &mut tx,
        &t,
        actual.get("settings").unwrap_or(&Value::Null),
    )
    .await?;
    validate_company_release(&mut tx, &t).await?;
    tx.commit().await?;
    Ok(json!({"id":release,"published":selected}))
}
