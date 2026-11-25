//! Clone only catalog/configuration into a private tenant; customer/order/payment state is excluded.
use super::live;
use super::snapshot::{Tx, snapshot};
use crate::ports::ReleaseHost;
use crate::store::staging_documents as documents;
use serde_json::{Value, json};
use sqlx::Row;
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
    let rows =
        sqlx::query("SELECT manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id")
            .bind(t)
            .fetch_all(&mut **tx)
            .await?;
    for r in rows {
        let m: Manifest =
            serde_json::from_value(r.get("manifest")).map_err(|_| bad("Invalid app"))?;
        if m.runtime == "declarative" {
            host.install_app(tx, id, m.clone()).await?;
            for entity in m.entities.iter().filter(|e| {
                configuration(e)
                    && (e.public_read
                        || (e.extends.is_some() && entity_access(&m, e, h, "list").is_ok()))
            }) {
                sqlx::query("SELECT set_config('rac.tenant',$1,true)")
                    .bind(t)
                    .execute(&mut **tx)
                    .await?;
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
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,16))")
        .bind(&t)
        .execute(&mut *tx)
        .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop_environments WHERE live_tenant=$1")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
    if count >= 10 {
        return Err(bad("Maximum 10 private environments per shop"));
    }
    lock_company(&mut tx, &t).await?;
    let base = snapshot(&mut tx, &t).await?;
    sqlx::query("INSERT INTO tenants(id,name) VALUES($1,$2)")
        .bind(&id)
        .bind(name)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO shop_environments(tenant,live_tenant,name,baseline) VALUES($1,$2,$3,$4)",
    )
    .bind(&id)
    .bind(&t)
    .bind(name)
    .bind(base)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO products SELECT (jsonb_populate_record(NULL::products,to_jsonb(p)||jsonb_build_object('tenant',$2::text))).* FROM products p WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO product_translations SELECT $2,product_id,language_id,name,description FROM product_translations WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO product_assets SELECT (jsonb_populate_record(NULL::product_assets,to_jsonb(a)||jsonb_build_object('tenant',$2::text))).* FROM product_assets a WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO categories SELECT $2,id,parent_id,position,data,revision FROM categories WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO product_categories SELECT $2,product_id,category_id FROM product_categories WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    for table in ["commerce_settings", "experiences", "receipt_settings"] {
        let sql =
            format!("INSERT INTO {table}(tenant,data) SELECT $2,data FROM {table} WHERE tenant=$1");
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(&t)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT INTO order_state_machines(tenant,data) SELECT $2,data FROM order_state_machines WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    // No external services or PSP configurations are registered for a sandbox tenant.
    sqlx::query("UPDATE commerce_settings SET data=jsonb_set(data,'{payments}',COALESCE((SELECT jsonb_agg(p) FROM jsonb_array_elements(data->'payments') p WHERE p->>'mode'<>'app'),'[]')) WHERE tenant=$1").bind(&id).execute(&mut *tx).await?;
    for table in ["commerce_promotions", "commerce_flows", "sales_channels"] {
        let sql = format!(
            "INSERT INTO {table}(tenant,id,data) SELECT $2,id,data FROM {table} WHERE tenant=$1"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(&t)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    b2b::clone_configuration(&mut tx, &t, &id).await?;
    clone_apps(host, &mut tx, &h, &t, &id).await?;
    sqlx::query("INSERT INTO policy(tenant,variant) VALUES($1,'discovery'),($1,'comparison')")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let wat = include_str!("../../../../../extensions/company-limit.wat");
    sqlx::query("INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3)")
        .bind(&id)
        .bind(wat)
        .bind(hash(wat))
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO commerce_rules(tenant,id,name,condition,active) SELECT $2,id,name,condition,active FROM commerce_rules WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO product_channel_visibility SELECT $2,product_id,channel_id,visible FROM product_channel_visibility WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO company_logos(tenant,id,content,digest,mime,width,height) SELECT $2,id,content,digest,mime,width,height FROM company_logos WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO company_overrides(tenant,channel_id,data) SELECT $2,channel_id,data FROM company_overrides WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO commerce_overrides(tenant,channel_id,data) SELECT $2,channel_id,data FROM commerce_overrides WHERE tenant=$1").bind(&t).bind(&id).execute(&mut *tx).await?;
    documents::clone_sources(&mut tx, &t, &id).await?;
    // Sandbox baseline is the actual sanitized clone, so disabled integrations aren't release changes.
    let stagebase = snapshot(&mut tx, &id).await?;
    let livebase = snapshot(&mut tx, &t).await?;
    sqlx::query("UPDATE shop_environments SET baseline=$1 WHERE tenant=$2")
        .bind(json!({"stage":stagebase,"live":livebase}))
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    if let Some(owner) = preview_owner {
        sqlx::query("UPDATE shop_environments SET preview_owner=$1,preview_until=now()+interval '1 hour' WHERE tenant=$2").bind(owner).bind(&id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    host.register_sandbox(&id, wat)?;
    Ok(
        json!({"id":id,"name":name,"private":true,"previewPath":format!("/?shop={id}&sandbox=1#"),"payments":"simulated-only"}),
    )
}
