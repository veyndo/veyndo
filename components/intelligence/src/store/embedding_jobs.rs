//! Durable bounded embedding jobs: short claims and revision fences keep provider latency outside PostgreSQL. Checked against the schema at compile time.
use veyndo_core::{Result, State, hash, uid};

/// A leased embedding job.
pub struct Job {
    pub tenant: String,
    pub kind: String,
    pub object_id: String,
    pub revision: i64,
    pub lease: String,
}

/// Queues the chunks of one document, or every product whose vector is missing or stale.
pub async fn enqueue(a: &State, t: &str, document: Option<&str>, model: &str) -> Result<i64> {
    let result = if document.is_some() {
        sqlx::query!("INSERT INTO embedding_jobs(tenant,kind,object_id) SELECT tenant,'document',document_id||':'||position FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2 AND (embedding IS NULL OR embedding_model<>$3 OR EXISTS(SELECT 1 FROM embedding_jobs j WHERE j.tenant=knowledge_chunks.tenant AND j.kind='document' AND j.object_id=knowledge_chunks.document_id||':'||knowledge_chunks.position)) ON CONFLICT(tenant,kind,object_id) DO UPDATE SET available_at=now(),attempts=0,error_code=NULL",
            t, document, model).execute(&a.db).await?
    } else {
        sqlx::query!("INSERT INTO embedding_jobs(tenant,kind,object_id) SELECT p.tenant,'product',p.id FROM products p LEFT JOIN semantic_products s ON s.tenant=p.tenant AND s.product_id=p.id WHERE p.tenant=$1 AND ($2::text IS NULL) AND (s.product_id IS NULL OR s.embedding_model<>$3 OR s.content_hash<>encode(sha256(convert_to(p.name||E'\\n'||p.category||E'\\n'||p.description,'UTF8')),'hex') OR EXISTS(SELECT 1 FROM embedding_jobs j WHERE j.tenant=p.tenant AND j.kind='product' AND j.object_id=p.id)) ON CONFLICT(tenant,kind,object_id) DO UPDATE SET available_at=now(),attempts=0,error_code=NULL",
            t, document, model).execute(&a.db).await?
    };
    Ok(result.rows_affected() as i64)
}
/// Leases one ready tenant's batch of jobs for the current generation.
pub async fn claim(a: &State, model: &str, model_revision: i64) -> Result<Vec<Job>> {
    let mut tx = a.db.begin().await?;
    let jobs = sqlx::query_file_as!(
        Job,
        "src/store/embedding_job_claim.sql",
        uid(),
        model,
        model_revision
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(jobs)
}

/// The text behind each leased job; a job whose source is gone is acknowledged.
pub async fn sources<'a>(
    a: &State,
    tenant: &str,
    jobs: &'a [Job],
) -> Result<Vec<(&'a Job, String)>> {
    let mut sources = Vec::new();
    for job in jobs {
        let id = &job.object_id;
        let text = if job.kind == "product" {
            sqlx::query_scalar!(r#"SELECT name||E'\n'||category||E'\n'||description AS "text!" FROM products WHERE tenant=$1 AND id=$2"#, tenant, id).fetch_optional(&a.db).await?
        } else {
            sqlx::query_scalar!("SELECT text FROM knowledge_chunks WHERE tenant=$1 AND document_id||':'||position=$2", tenant, id).fetch_optional(&a.db).await?
        };
        if let Some(text) = text {
            sources.push((job, text));
        } else {
            acknowledge(a, job).await?;
        }
    }
    Ok(sources)
}

/// Stores the vectors whose job lease, source text and generation are still current.
pub async fn publish(
    a: &State,
    tenant: &str,
    model: &str,
    model_revision: i64,
    sources: Vec<(&Job, String)>,
    vectors: Vec<Vec<f32>>,
) -> Result<()> {
    for ((job, text), vector) in sources.into_iter().zip(vectors) {
        let mut tx = a.db.begin().await?;
        // Lock the generation before publishing a result from the external provider.
        // A newer worker can advance it while inference is running.
        let generation_current = sqlx::query_scalar!(
                    r#"SELECT EXISTS(SELECT 1 FROM embedding_generations
                     WHERE tenant=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1)
                       AND model=$2 AND model_revision=$3 FOR SHARE) AS "current!""#,
                    tenant,
                    model,
                    model_revision
                )
                .fetch_one(&mut *tx)
                .await?;
        if !generation_current {
            break;
        }
        // Match source mutation lock order (source first, queue second).
        if job.kind == "product" {
            sqlx::query_scalar!(
                "SELECT id FROM products WHERE tenant=$1 AND id=$2 FOR SHARE",
                tenant,
                job.object_id
            )
            .fetch_optional(&mut *tx)
            .await?;
        } else {
            sqlx::query_scalar!("SELECT position FROM knowledge_chunks WHERE tenant=$1 AND document_id||':'||position=$2 FOR SHARE",
                tenant, job.object_id).fetch_optional(&mut *tx).await?;
        }
        let current = sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM embedding_jobs WHERE tenant=$1 AND kind=$2 AND object_id=$3 AND revision=$4 AND lease=$5 AND lease_until>now() FOR UPDATE) AS "current!""#,
                tenant, job.kind, job.object_id, job.revision, job.lease).fetch_one(&mut *tx).await?;
        if !current {
            continue;
        }
        if job.kind == "product" {
            sqlx::query!("INSERT INTO semantic_products(tenant,product_id,revision,embedding,embedding_model,content_hash) SELECT tenant,id,revision,$3,$4,$5 FROM products WHERE tenant=$1 AND id=$2 AND name||E'\\n'||category||E'\\n'||description=$6 ON CONFLICT(tenant,product_id) DO UPDATE SET revision=EXCLUDED.revision,embedding=EXCLUDED.embedding,embedding_model=EXCLUDED.embedding_model,content_hash=EXCLUDED.content_hash,updated_at=now()",
                    tenant, job.object_id, &vector, model, hash(&text), text).execute(&mut *tx).await?;
        } else {
            sqlx::query!("UPDATE knowledge_chunks SET embedding=$3,embedding_model=$4 WHERE tenant=$1 AND document_id||':'||position=$2 AND text=$5",
                    tenant, job.object_id, &vector, model, text).execute(&mut *tx).await?;
        }
        sqlx::query!("DELETE FROM embedding_jobs WHERE tenant=$1 AND kind=$2 AND object_id=$3 AND revision=$4 AND lease=$5",
                tenant, job.kind, job.object_id, job.revision, job.lease).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok(())
}

/// Releases the leases with a classified error and a bounded backoff; busy admission costs no attempt.
pub async fn fail(
    a: &State,
    tenant: &str,
    sources: Vec<(&Job, String)>,
    error: &str,
) -> Result<()> {
    let code = if error.contains("admission") {
        "embedding_admission_busy"
    } else if error.contains("limit") {
        "embedding_source_limit"
    } else if error.contains("HTTP") {
        "embedding_provider_rejected"
    } else if error.contains("mismatch") || error.contains("Invalid") || error.contains("Missing") {
        "embedding_invalid_response"
    } else {
        "embedding_provider_unavailable"
    };
    for (job, _) in sources {
        sqlx::query!("UPDATE embedding_jobs SET attempts=attempts+CASE WHEN $6='embedding_admission_busy' THEN 0 ELSE 1 END,error_code=$6,lease=NULL,lease_until=NULL,available_at=now()+least(300,power(2,attempts+1)::int)*interval '1 second' WHERE tenant=$1 AND kind=$2 AND object_id=$3 AND revision=$4 AND lease=$5",
                tenant, job.kind, job.object_id, job.revision, job.lease, code).execute(&a.db).await?;
    }
    Ok(())
}

/// Removes a job whose source is gone, if it still holds the lease.
async fn acknowledge(a: &State, job: &Job) -> Result<()> {
    sqlx::query!("DELETE FROM embedding_jobs WHERE tenant=$1 AND kind=$2 AND object_id=$3 AND revision=$4 AND lease=$5",
        job.tenant, job.kind, job.object_id, job.revision, job.lease).execute(&a.db).await?;
    Ok(())
}
