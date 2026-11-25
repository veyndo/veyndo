//! SQL of a staging clone: the sandbox shop row and its copied catalogue/configuration; customer, order and payment state never clones.
use super::staging_documents::Tx;
use serde_json::Value;
use sqlx::Row;
use veyndo_core::Result;

/// Manifests of the live shop's active apps, by ID.
pub async fn active_manifests(tx: &mut Tx<'_>, t: &str) -> Result<Vec<Value>> {
    let rows =
        sqlx::query("SELECT manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id")
            .bind(t)
            .fetch_all(&mut **tx)
            .await?;
    Ok(rows.iter().map(|r| r.get("manifest")).collect())
}

/// Scopes app record row-level security to a shop for the rest of the transaction.
pub async fn scope_app_records(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query("SELECT set_config('rac.tenant',$1,true)")
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Serializes environment creation and release for one live shop.
pub async fn lock_environments(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,16))")
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// How many private environments the live shop has.
pub async fn environment_count(tx: &mut Tx<'_>, t: &str) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM shop_environments WHERE live_tenant=$1")
            .bind(t)
            .fetch_one(&mut **tx)
            .await?,
    )
}

/// Creates the sandbox shop and its environment row with the given baseline.
pub async fn insert_environment(
    tx: &mut Tx<'_>,
    live: &str,
    id: &str,
    name: &str,
    baseline: &Value,
) -> Result<()> {
    sqlx::query("INSERT INTO tenants(id,name) VALUES($1,$2)")
        .bind(id)
        .bind(name)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO shop_environments(tenant,live_tenant,name,baseline) VALUES($1,$2,$3,$4)",
    )
    .bind(id)
    .bind(live)
    .bind(name)
    .bind(baseline)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Copies the live catalogue (products, translations, assets, categories) into the sandbox.
pub async fn copy_catalog(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    sqlx::query("INSERT INTO products SELECT (jsonb_populate_record(NULL::products,to_jsonb(p)||jsonb_build_object('tenant',$2::text))).* FROM products p WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO product_translations SELECT $2,product_id,language_id,name,description FROM product_translations WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO product_assets SELECT (jsonb_populate_record(NULL::product_assets,to_jsonb(a)||jsonb_build_object('tenant',$2::text))).* FROM product_assets a WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO categories SELECT $2,id,parent_id,position,data,revision FROM categories WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO product_categories SELECT $2,product_id,category_id FROM product_categories WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    Ok(())
}

/// Copies settings, experience, receipts and the order workflow; app payment methods are dropped.
pub async fn copy_settings(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    for table in ["commerce_settings", "experiences", "receipt_settings"] {
        let sql =
            format!("INSERT INTO {table}(tenant,data) SELECT $2,data FROM {table} WHERE tenant=$1");
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(t)
            .bind(id)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query("INSERT INTO order_state_machines(tenant,data) SELECT $2,data FROM order_state_machines WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    // No external services or PSP configurations are registered for a sandbox tenant.
    sqlx::query("UPDATE commerce_settings SET data=jsonb_set(data,'{payments}',COALESCE((SELECT jsonb_agg(p) FROM jsonb_array_elements(data->'payments') p WHERE p->>'mode'<>'app'),'[]')) WHERE tenant=$1").bind(id).execute(&mut **tx).await?;
    for table in ["commerce_promotions", "commerce_flows", "sales_channels"] {
        let sql = format!(
            "INSERT INTO {table}(tenant,id,data) SELECT $2,id,data FROM {table} WHERE tenant=$1"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(t)
            .bind(id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// The sandbox's discovery/comparison policies and its company-limit extension.
pub async fn insert_defaults(tx: &mut Tx<'_>, id: &str, wat: &str, digest: &str) -> Result<()> {
    sqlx::query("INSERT INTO policy(tenant,variant) VALUES($1,'discovery'),($1,'comparison')")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3)")
        .bind(id)
        .bind(wat)
        .bind(digest)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Copies rules, channel visibility, company logos/overrides and channel overrides.
pub async fn copy_channel_configuration(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    sqlx::query("INSERT INTO commerce_rules(tenant,id,name,condition,active) SELECT $2,id,name,condition,active FROM commerce_rules WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO product_channel_visibility SELECT $2,product_id,channel_id,visible FROM product_channel_visibility WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO company_logos(tenant,id,content,digest,mime,width,height) SELECT $2,id,content,digest,mime,width,height FROM company_logos WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO company_overrides(tenant,channel_id,data) SELECT $2,channel_id,data FROM company_overrides WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO commerce_overrides(tenant,channel_id,data) SELECT $2,channel_id,data FROM commerce_overrides WHERE tenant=$1").bind(t).bind(id).execute(&mut **tx).await?;
    Ok(())
}

/// Replaces the environment's baseline.
pub async fn set_baseline(tx: &mut Tx<'_>, id: &str, baseline: &Value) -> Result<()> {
    sqlx::query("UPDATE shop_environments SET baseline=$1 WHERE tenant=$2")
        .bind(baseline)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Marks the environment as a developer preview owned by one actor for one hour.
pub async fn mark_preview(tx: &mut Tx<'_>, id: &str, owner: &str) -> Result<()> {
    sqlx::query("UPDATE shop_environments SET preview_owner=$1,preview_until=now()+interval '1 hour' WHERE tenant=$2").bind(owner).bind(id).execute(&mut **tx).await?;
    Ok(())
}
