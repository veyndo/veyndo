//! Knowledge sources: hash-bound chunks, revision-checked edits, archive and publication, written atomically with their graph relations.
use crate::domain::source_content::admit_guidance;
use crate::knowledge::{sync_document, sync_product};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::store::products;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, history_context, uid,
};

/// Rebuilds the chunks of a source from its content and translations; returns how many were written.
pub async fn chunks(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    v: &Value,
) -> Result<usize> {
    sqlx::query("DELETE FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2")
        .bind(t)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let mut sources = vec![(
        v["locale"].as_str().unwrap(),
        v["content"].as_str().unwrap(),
    )];
    for (locale, fields) in v["translations"].as_object().unwrap() {
        if let Some(text) = fields["content"].as_str() {
            sources.push((locale, text));
        }
    }
    let mut position = 0;
    for (locale, text) in sources {
        for chunk in text.chars().collect::<Vec<_>>().chunks(1200) {
            sqlx::query("INSERT INTO knowledge_chunks(tenant,document_id,position,text,locale) VALUES($1,$2,$3,$4,$5)").bind(t).bind(id).bind(position as i32).bind(chunk.iter().collect::<String>()).bind(locale).execute(&mut **tx).await?;
            position += 1;
        }
    }
    Ok(position)
}

/// One source with every stored field except the tenant.
pub async fn detail(a: &State, t: &str, id: &str) -> Result<Value> {
    let v: Value = sqlx::query_scalar(
        "SELECT to_jsonb(d)-'tenant' FROM knowledge_documents d WHERE tenant=$1 AND id=$2",
    )
    .bind(t)
    .bind(id)
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Source unavailable".into()))?;
    Ok(v)
}

/// Replaces a source at the expected revision; an edit always returns it to private review.
pub async fn edit(
    a: &State,
    h: &RequestContext,
    t: &str,
    id: &str,
    v: &Value,
    data: &Value,
) -> Result<Value> {
    let rev = v["revision"].as_i64().ok_or(bad("Revision required"))?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,15))")
        .bind(t)
        .execute(&mut *tx)
        .await?;
    let prior: String = sqlx::query_scalar("SELECT kind FROM knowledge_documents WHERE tenant=$1 AND id=$2 AND NOT archived FOR UPDATE").bind(t).bind(id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Source unavailable".into()))?;
    admit_guidance(h, &json!(prior))?;
    let product = data["productId"].as_str();
    if let Some(p) = product {
        let r = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(p)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(bad("Unknown owning product"))?;
        sync_product(&mut tx, t, &json!(products::product(&r))).await?;
    }
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM knowledge_documents WHERE tenant=$1 AND id<>$2 AND product_id IS NOT DISTINCT FROM $3 AND content_hash=$4)").bind(t).bind(id).bind(product).bind(data["digest"].as_str()).fetch_one(&mut *tx).await?;
    if duplicate {
        return Err(conflict("Identical source already exists"));
    }
    let n=sqlx::query("UPDATE knowledge_documents SET title=$1,content=$2,content_hash=$3,product_id=$4,kind=$5,locale=$6,translations=$7,visibility='private',revision=revision+1 WHERE tenant=$8 AND id=$9 AND revision=$10 AND NOT archived")
        .bind(data["title"].as_str()).bind(data["content"].as_str()).bind(data["digest"].as_str()).bind(product).bind(data["kind"].as_str()).bind(data["locale"].as_str()).bind(&data["translations"]).bind(t).bind(id).bind(rev).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Source changed, archived or unavailable"));
    }
    let chunks = chunks(&mut tx, t, id, data).await?;
    sync_document(
        &mut tx,
        t,
        id,
        product,
        data["title"].as_str().unwrap(),
        data["digest"].as_str().unwrap(),
    )
    .await?;
    record(
        &mut tx,
        t,
        id,
        "knowledge.document.updated",
        json!({"revision":rev+1,"visibility":"private"}),
    )
    .await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"revision":rev+1,"visibility":"private","chunks":chunks,"contentHash":data["digest"]}),
    )
}

/// Archives or restores a source after an explicit decision.
pub async fn lifecycle(
    a: &State,
    h: &RequestContext,
    t: &str,
    id: &str,
    v: &Value,
) -> Result<Value> {
    if v["approve"] != true {
        return Err(bad("Explicit decision required"));
    }
    let archived = v["archived"]
        .as_bool()
        .ok_or(bad("Archived must be boolean"))?;
    let rev = v["revision"].as_i64().ok_or(bad("Revision required"))?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let n=sqlx::query("UPDATE knowledge_documents SET archived=$1,visibility='private',revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4 AND archived<>$1").bind(archived).bind(t).bind(id).bind(rev).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Source changed or unavailable"));
    }
    record(
        &mut tx,
        t,
        id,
        if archived {
            "knowledge.document.archived"
        } else {
            "knowledge.document.restored"
        },
        json!({"revision":rev+1}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"revision":rev+1,"archived":archived,"visibility":"private"}))
}

/// Announces a decision about a source.
pub async fn record(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    kind: &str,
    data: Value,
) -> Result<()> {
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,$2,$3)")
        .bind(t)
        .bind(kind)
        .bind(json!({"documentId":id,"decision":data}))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Stores a validated source once; identical content for the same product returns the existing source.
pub async fn save(
    a: &State,
    h: &RequestContext,
    t: &str,
    data: &Value,
    source: &str,
) -> Result<Value> {
    let title = data["title"].as_str().unwrap();
    let text = data["content"].as_str().unwrap();
    let product = data["productId"].as_str();
    let digest = data["digest"].as_str().unwrap();
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,15))")
        .bind(t)
        .execute(&mut *tx)
        .await?;
    if let Some(product) = product {
        let row = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(product)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(bad("Unknown owning product"))?;
        sync_product(&mut tx, t, &json!(products::product(&row))).await?;
    }
    if let Some(id)=sqlx::query_scalar::<_,String>("SELECT id FROM knowledge_documents WHERE tenant=$1 AND product_id IS NOT DISTINCT FROM $2 AND content_hash=$3").bind(t).bind(product).bind(digest).fetch_optional(&mut *tx).await? {return Ok(json!({"id":id,"reused":true,"visibility":"private-or-existing"}));}
    let id = uid();
    sqlx::query("INSERT INTO knowledge_documents(tenant,id,product_id,title,content_hash,content,source_type,kind,locale,translations) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(t).bind(&id).bind(product).bind(title).bind(digest).bind(text).bind(source).bind(data["kind"].as_str()).bind(data["locale"].as_str()).bind(&data["translations"]).execute(&mut *tx).await?;
    let chunks = chunks(&mut tx, t, &id, data).await?;
    sync_document(&mut tx, t, &id, product, title, digest).await?;
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'knowledge.document.ingested',$2)")
        .bind(t)
        .bind(json!({"documentId":id,"productId":product,"sourceType":source}))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"contentHash":digest,"revision":1,"visibility":"private","chunks":chunks,"sourceType":source}),
    )
}

/// The most recent sources of a shop.
pub async fn list(a: &State, t: &str) -> Result<Value> {
    let rows=sqlx::query("SELECT id,product_id,title,source_type,visibility,revision,content_hash FROM knowledge_documents WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100").bind(t).fetch_all(&a.db).await?;
    Ok(
        json!({"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"productId":r.get::<Option<String>,_>("product_id"),"title":r.get::<String,_>("title"),"sourceType":r.get::<String,_>("source_type"),"visibility":r.get::<String,_>("visibility"),"revision":r.get::<i64,_>("revision"),"contentHash":r.get::<String,_>("content_hash")})).collect::<Vec<_>>() }),
    )
}

/// Changes the visibility of a source after an explicit publication decision.
pub async fn publish(a: &State, h: &RequestContext, t: &str, id: &str, v: &Value) -> Result<Value> {
    let visibility = v["visibility"]
        .as_str()
        .filter(|v| ["private", "public"].contains(v))
        .ok_or(bad("Visibility must be private/public"))?;
    if v["approve"] != true {
        return Err(bad("Explicit publication decision required"));
    }
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let n=sqlx::query("UPDATE knowledge_documents SET visibility=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4 AND NOT archived AND (kind<>'guidance' OR $1='private')").bind(visibility).bind(t).bind(id).bind(v["revision"].as_i64()).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Document changed or unavailable"));
    }
    record(
        &mut tx,
        t,
        id,
        "knowledge.document.visibility",
        json!({"visibility":visibility}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"updated":true,"id":id,"visibility":visibility}))
}

/// Re-admit the exact supplied evidence, independent of index readiness or search ranking.
pub async fn sources_current(a: &State, t: &str, sources: &Value) -> Result<bool> {
    let Some(items) = sources.as_array().filter(|items| items.len() <= 9) else {
        return Ok(false);
    };
    let mut records = Vec::new();
    for item in items {
        if item["omittedRecords"].as_u64().is_some()
            && item["readToolsRequired"] == true
            && item.as_object().is_some_and(|o| o.len() == 2)
        {
            continue;
        }
        if !["sourceId", "documentId", "contentHash", "locale", "text"]
            .iter()
            .all(|key| item[*key].is_string())
            || item["revision"].as_i64().is_none_or(|r| r <= 0)
            || !item
                .get("productId")
                .is_some_and(|v| v.is_null() || v.is_string())
        {
            return Ok(false);
        }
        records.push(item.clone());
    }
    sqlx::query_scalar(
        "SELECT NOT EXISTS(SELECT 1 FROM jsonb_to_recordset($2) AS s(\"sourceId\" text,\"documentId\" text,\"contentHash\" text,\"productId\" text,revision bigint,locale text,text text) WHERE NOT EXISTS(SELECT 1 FROM knowledge_documents d JOIN knowledge_chunks c ON c.tenant=d.tenant AND c.document_id=d.id WHERE d.tenant=$1 AND d.id=s.\"documentId\" AND NOT d.archived AND d.revision=s.revision AND d.content_hash=s.\"contentHash\" AND d.product_id IS NOT DISTINCT FROM s.\"productId\" AND d.id||':'||c.position=s.\"sourceId\" AND c.text=s.text AND c.locale=s.locale))",
    ).bind(t).bind(json!(records)).fetch_one(&a.db).await.map_err(Into::into)
}
