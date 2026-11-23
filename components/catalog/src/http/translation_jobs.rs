//! Resumable translation jobs: enqueueing a job for an enabled target language, the job list, its paginated drafts and resume/cancel controls.
use crate::service::translation_apply::admit_selection;
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashMap;
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::{merchant, permit};

/// Both explicit settings commands and approved merchant tasks enqueue the same durable draft owner; the target must be an enabled language other than the main one.
pub async fn enqueue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    languages: (&str, &[String]),
    request: (&str, &Value, bool, Option<&[String]>),
) -> Result<()> {
    let (target, choice, overwrite, products) = request;
    let (main_locale, locales) = languages;
    admit_selection(tx, t, products).await?;
    if !locales.iter().any(|l| l == target) || target == main_locale {
        return Err(bad("Select an enabled translation target"));
    }
    let row = sqlx::query(
        "SELECT count(*) AS total,coalesce(max(id),'') AS highwater FROM products WHERE tenant=$1 AND ($2::text[] IS NULL OR id=ANY($2))",
    )
    .bind(t)
    .bind(products)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query("INSERT INTO translation_jobs(tenant,id,source_locale,target_locale,choice,overwrite,total,highwater,product_ids) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(t).bind(id).bind(main_locale).bind(target).bind(choice).bind(overwrite).bind(row.get::<i64,_>("total")).bind(row.get::<String,_>("highwater")).bind(products).execute(&mut **tx).await?;
    Ok(())
}

/// `GET /api/merchant/translations`: the latest translation jobs.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let t = merchant(&h)?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(j)-'tenant'-'lease'-'lease_until' FROM translation_jobs j WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30").bind(&t).fetch_all(&a.db).await?;
    Ok(Json(json!({"jobs":rows})))
}
/// `GET /api/merchant/translations/{id}`: a job with its status counts and a page of drafts.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let t = merchant(&h)?;
    let job:Value=sqlx::query_scalar("SELECT to_jsonb(j)-'tenant'-'lease'-'lease_until' FROM translation_jobs j WHERE tenant=$1 AND id=$2").bind(&t).bind(&id).fetch_optional(&a.db).await?.ok_or(bad("Translation job unavailable"))?;
    let items:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(i)-'tenant'-'job_id' FROM translation_items i WHERE tenant=$1 AND job_id=$2 AND product_id>$3 ORDER BY product_id LIMIT 51").bind(&t).bind(&id).bind(q.get("cursor").map(String::as_str).unwrap_or("")).fetch_all(&a.db).await?;
    let counts:Value=sqlx::query_scalar("SELECT coalesce(jsonb_object_agg(status,n),'{}') FROM (SELECT status,count(*) AS n FROM translation_items WHERE tenant=$1 AND job_id=$2 GROUP BY status) s").bind(&t).bind(&id).fetch_one(&a.db).await?;
    Ok(Json(
        json!({"job":job,"counts":counts,"items":items.iter().take(50).collect::<Vec<_>>(),"nextCursor":if items.len()>50 {items[49]["product_id"].clone()}else{Value::Null}}),
    ))
}
/// `PUT /api/merchant/translations/{id}`: resume or cancel a job that no task controls.
pub async fn control(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    permit(&h, "catalog")?;
    let t = merchant(&h)?;
    let status = match v["action"].as_str() {
        Some("resume") => "queued",
        Some("cancel") => "cancelled",
        _ => return Err(bad("Invalid translation action")),
    };
    let n=sqlx::query("UPDATE translation_jobs SET status=$1,error=NULL,lease=NULL,lease_until=NULL WHERE tenant=$2 AND id=$3 AND run_task IS NULL AND status IN ('queued','failed','processing')").bind(status).bind(&t).bind(&id).execute(&a.db).await?.rows_affected();
    if n == 0 {
        return Err(conflict(
            "Translation is complete or controlled by its original task; use the task continuation controls",
        ));
    }
    Ok(Json(json!({"status":status})))
}
