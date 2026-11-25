//! Clone only catalog/configuration into a private tenant; customer/order/payment state is excluded.
use super::live;
use super::snapshot::{Tx, snapshot};
use crate::ports::ReleaseHost;
use crate::store::staging_clone as clone;
use crate::store::staging_documents as documents;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::domain::core_extensions::configuration;
use veyndo_app_platform::domain::entity_access::entity_access;
use veyndo_app_platform::store::records::{get as get_record, rows as record_rows};
use veyndo_b2b::store::staging as b2b;
use veyndo_core::store::history_context::context;
use veyndo_core::{RequestContext, Result, bad, hash, uid};
use veyndo_identity::permit;
use veyndo_orders::store::company::lock as lock_company;
/// Clones the live shop's catalogue and configuration into a new private environment.
pub async fn create<H: ReleaseHost>(host: &H, h: RequestContext, v: Value) -> Result<Value> {
    create_inner(host, h, v, None).await
}
/// Clones a short-lived developer preview environment owned by one actor.
pub async fn create_preview<H: ReleaseHost>(
    host: &H,
    h: RequestContext,
    name: &str,
    actor: &str,
) -> Result<Value> {
    create_inner(host, h, json!({"name":name}), Some(actor.to_owned())).await
}
/// Installs the live shop's declarative apps in the sandbox and copies their readable configuration records.
async fn clone_apps<H: ReleaseHost>(
    host: &H,
    tx: &mut Tx<'_>,
    h: &RequestContext,
    t: &str,
    id: &str,
) -> Result<()> {
    for manifest in clone::active_manifests(tx, t).await? {
        let m: Manifest = serde_json::from_value(manifest).map_err(|_| bad("Invalid app"))?;
        if m.runtime == "declarative" {
            host.install_app(tx, id, m.clone()).await?;
            for entity in m.entities.iter().filter(|e| {
                configuration(e)
                    && (e.public_read
                        || (e.extends.is_some() && entity_access(&m, e, h, "list").is_ok()))
            }) {
                clone::scope_app_records(tx, t).await?;
                let records = record_rows(tx, t, &m.id, entity, 1001).await?;
                if records.len() > 1000 {
                    return Err(bad(
                        "Prototype sandbox supports at most 1000 public records per app entity",
                    ));
                }
                for r in records {
                    let mut fields = r;
                    fields.as_object_mut().unwrap().remove("revision");
                    let record = fields.as_object_mut().unwrap().remove("id").unwrap();
                    // Installation may have seeded this exact configuration record. Update
                    // it through its actual CAS instead of colliding with revision zero.
                    let revision = get_record(
                        tx,
                        id,
                        &m.id,
                        &entity.name,
                        record.as_str().ok_or(bad("Invalid app record ID"))?,
                        false,
                    )
                    .await?
                    .and_then(|value| value["revision"].as_i64())
                    .unwrap_or(0);
                    host.save_record(
                        tx,
                        id,
                        &m,
                        entity,
                        &json!({"id":record,"revision":revision,"fields":fields}),
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}
/// The shared clone of an environment.
async fn create_inner<H: ReleaseHost>(
    host: &H,
    h: RequestContext,
    v: Value,
    preview_owner: Option<String>,
) -> Result<Value> {
    let a = host.core();
    permit(
        &h,
        if preview_owner.is_some() {
            "developer.manage"
        } else {
            "environments.manage"
        },
    )?;
    let t = live(a, &h).await?;
    let name = v["name"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 80)
        .ok_or(bad("Environment name required, maximum 80 bytes"))?;
    let id = format!("stage-{}", &uid()[..20]);
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "staging.clone").await?;
    clone::lock_environments(&mut tx, &t).await?;
    let count = clone::environment_count(&mut tx, &t).await?;
    if count >= 10 {
        return Err(bad("Maximum 10 private environments per shop"));
    }
    lock_company(&mut tx, &t).await?;
    let base = snapshot(&mut tx, &t).await?;
    clone::insert_environment(&mut tx, &t, &id, name, &base).await?;
    clone::copy_catalog(&mut tx, &t, &id).await?;
    clone::copy_settings(&mut tx, &t, &id).await?;
    b2b::clone_configuration(&mut tx, &t, &id).await?;
    clone_apps(host, &mut tx, &h, &t, &id).await?;
    let wat = include_str!("../../../../../extensions/company-limit.wat");
    clone::insert_defaults(&mut tx, &id, wat, &hash(wat)).await?;
    clone::copy_channel_configuration(&mut tx, &t, &id).await?;
    documents::clone_sources(&mut tx, &t, &id).await?;
    // Sandbox baseline is the actual sanitized clone, so disabled integrations aren't release changes.
    let stagebase = snapshot(&mut tx, &id).await?;
    let livebase = snapshot(&mut tx, &t).await?;
    clone::set_baseline(&mut tx, &id, &json!({"stage":stagebase,"live":livebase})).await?;
    if let Some(owner) = preview_owner {
        clone::mark_preview(&mut tx, &id, &owner).await?;
    }
    tx.commit().await?;
    host.register_sandbox(&id, wat)?;
    Ok(
        json!({"id":id,"name":name,"private":true,"previewPath":format!("/?shop={id}&sandbox=1#"),"payments":"simulated-only"}),
    )
}
