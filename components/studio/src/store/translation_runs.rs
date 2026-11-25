//! SQL of the catalogue translation worker: job leases, keyset product reads and reviewed translation items.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::{Result, State};

/// Fails expired leases of task-linked jobs, which must be resumed explicitly after a possibly charged call.
pub async fn interrupt_expired(a: &State) -> Result<()> {
    sqlx::query!("UPDATE translation_jobs SET status='failed',error_code='call_interrupted',error='Translation call interrupted; explicitly resume the parent task',lease=NULL,lease_until=NULL WHERE (tenant,id) IN (SELECT tenant,id FROM translation_jobs WHERE run_task IS NOT NULL AND status='processing' AND lease_until<now() ORDER BY created_at LIMIT 20 FOR UPDATE SKIP LOCKED)").execute(&a.db).await?;
    Ok(())
}

/// A leased translation job.
pub struct Job {
    pub tenant: String,
    pub id: String,
    pub source_locale: String,
    pub target_locale: String,
    pub choice: Value,
    pub overwrite: bool,
    pub cursor: String,
    pub highwater: String,
    pub total: i64,
    pub processed: i64,
    pub product_ids: Option<Vec<String>>,
    pub run_task: Option<String>,
}

/// Leases the oldest runnable job of an active shop.
pub async fn lease_job(a: &State, lease: &str) -> Result<Option<Job>> {
    Ok(sqlx::query_as!(Job, "UPDATE translation_jobs SET status='processing',lease=$1,lease_until=now()+interval '15 minutes' WHERE (tenant,id)=(SELECT tenant,id FROM translation_jobs WHERE (status='queued' OR (status='processing' AND run_task IS NULL AND lease_until<now())) AND EXISTS(SELECT 1 FROM tenants t WHERE t.id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=translation_jobs.tenant),translation_jobs.tenant) AND t.status='active') ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING tenant,id,source_locale,target_locale,choice,overwrite,cursor,highwater,total,processed,product_ids,run_task", lease).fetch_optional(&a.db).await?)
}

/// Fails the leased job with an error message.
pub async fn fail(a: &State, t: &str, id: &str, lease: &str, error: &str) -> Result<()> {
    sqlx::query!("UPDATE translation_jobs SET status='failed',error=$1,lease=NULL,lease_until=NULL WHERE tenant=$2 AND id=$3 AND lease=$4", error, t, id, lease).execute(&a.db).await?;
    Ok(())
}

/// Wakes the queued parent task once all of its translation jobs have stopped.
pub async fn wake_task(a: &State, t: &str, task: &str) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_after=now() WHERE tenant=$1 AND id=$2 AND run_state='queued' AND EXISTS(SELECT 1 FROM translation_jobs j WHERE j.tenant=$1 AND j.run_task=$2 AND j.status IN ('ready','failed','cancelled'))", t, task).execute(&a.db).await?;
    Ok(())
}

/// The job's position: the keyset window, its locales and the optional product selection.
pub struct Window<'a> {
    pub cursor: &'a str,
    pub highwater: &'a str,
    pub source: &'a str,
    pub target: &'a str,
    pub products: Option<Vec<String>>,
}

/// The product a translation step reads, with its source and target translations.
pub struct NextProduct {
    pub id: String,
    pub revision: i64,
    pub name: String,
    pub description: String,
    pub extra: Value,
    pub translated_name: Option<String>,
    pub translated_description: Option<String>,
    pub target_name: Option<String>,
}

/// The next product after the cursor with its source and target translations.
pub async fn next_product(a: &State, t: &str, w: Window<'_>) -> Result<Option<NextProduct>> {
    Ok(sqlx::query_as!(NextProduct, r#"SELECT p.id,p.revision,p.name,p.description,p.extra,s.name AS "translated_name?",s.description AS "translated_description?",tt.name AS "target_name?" FROM products p LEFT JOIN product_translations s ON s.tenant=p.tenant AND s.product_id=p.id AND s.language_id=(SELECT id FROM languages WHERE locale=$4) LEFT JOIN product_translations tt ON tt.tenant=p.tenant AND tt.product_id=p.id AND tt.language_id=(SELECT id FROM languages WHERE locale=$5) WHERE p.tenant=$1 AND p.id>$2 AND p.id<=$3 AND ($6::text[] IS NULL OR p.id=ANY($6)) ORDER BY p.id LIMIT 1"#, t, w.cursor, w.highwater, w.source, w.target, w.products.as_deref()).fetch_optional(&a.db).await?)
}

/// Finishes the leased job.
pub async fn finish(a: &State, t: &str, id: &str, lease: &str) -> Result<()> {
    sqlx::query!("UPDATE translation_jobs SET status='ready',lease=NULL,lease_until=NULL WHERE tenant=$1 AND id=$2 AND lease=$3", t, id, lease).execute(&a.db).await?;
    Ok(())
}

/// Whether the product already has a target translation with a description.
pub async fn has_target_description(
    a: &State,
    t: &str,
    product: &str,
    target: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM product_translations WHERE tenant=$1 AND product_id=$2 AND language_id=(SELECT id FROM languages WHERE locale=$3) AND description IS NOT NULL) AS "exists!""#, t, product, target).fetch_one(&a.db).await?)
}

/// The current lease of a processing job, locked.
pub async fn current_lease(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!("SELECT lease FROM translation_jobs WHERE tenant=$1 AND id=$2 AND status='processing' FOR UPDATE", t, id).fetch_optional(&mut **tx).await?.flatten())
}

/// One translated (or skipped) product of a job.
pub struct TranslationItem<'a> {
    pub product: String,
    pub revision: i64,
    pub source: Value,
    pub result: Value,
    pub status: &'a str,
    pub evidence: Option<Value>,
}

/// Stores a translation item once and advances the job's cursor past its product.
pub async fn store_item(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    lease: &str,
    item: TranslationItem<'_>,
) -> Result<()> {
    sqlx::query!("INSERT INTO translation_items(tenant,job_id,product_id,revision,source,result,status,evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING", t, id, &item.product, item.revision, item.source, item.result, item.status, item.evidence).execute(&mut **tx).await?;
    sqlx::query!("UPDATE translation_jobs SET cursor=$1,processed=processed+1,status='queued',lease=NULL,lease_until=NULL WHERE tenant=$2 AND id=$3 AND lease=$4", &item.product, t, id, lease).execute(&mut **tx).await?;
    Ok(())
}
