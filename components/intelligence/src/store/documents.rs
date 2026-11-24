//! Knowledge sources: hash-bound chunks, revision-checked edits, archive and publication, written atomically with their graph relations.
use crate::domain::source_content::{Source, admit_guidance};
use crate::knowledge::{sync_document, sync_product, vectors};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::store::products;
use veyndo_core::document::{lenient, view};
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, history_context, uid,
};

/// A merchant decision on a source: its expected revision, approval, archive state or visibility.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SourceDecision {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    archived: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    visibility: Option<String>,
}

/// Rebuilds the chunks of a source from its content and translations; returns how many were written.
pub async fn chunks(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    source: &Source,
) -> Result<usize> {
    sqlx::query("DELETE FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2")
        .bind(t)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let mut position = 0;
    for (locale, text) in source.texts() {
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
    data: &Source,
) -> Result<Value> {
    let rev = view::<SourceDecision>(v)
        .revision
        .ok_or(bad("Revision required"))?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,15))")
        .bind(t)
        .execute(&mut *tx)
        .await?;
    let prior: String = sqlx::query_scalar("SELECT kind FROM knowledge_documents WHERE tenant=$1 AND id=$2 AND NOT archived FOR UPDATE").bind(t).bind(id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Source unavailable".into()))?;
    admit_guidance(h, &json!(prior))?;
    let product = data.product_id.as_deref();
    if let Some(p) = product {
        let r = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(p)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(bad("Unknown owning product"))?;
        sync_product(&mut tx, t, &json!(products::product(&r))).await?;
    }
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM knowledge_documents WHERE tenant=$1 AND id<>$2 AND product_id IS NOT DISTINCT FROM $3 AND content_hash=$4)").bind(t).bind(id).bind(product).bind(&data.digest).fetch_one(&mut *tx).await?;
    if duplicate {
        return Err(conflict("Identical source already exists"));
    }
    let n=sqlx::query("UPDATE knowledge_documents SET title=$1,content=$2,content_hash=$3,product_id=$4,kind=$5,locale=$6,translations=$7,visibility='private',revision=revision+1 WHERE tenant=$8 AND id=$9 AND revision=$10 AND NOT archived")
        .bind(&data.title).bind(&data.content).bind(&data.digest).bind(product).bind(&data.kind).bind(&data.locale).bind(&data.translations).bind(t).bind(id).bind(rev).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Source changed, archived or unavailable"));
    }
    let chunks = chunks(&mut tx, t, id, data).await?;
    sync_document(&mut tx, t, id, product, &data.title, &data.digest).await?;
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
        json!({"id":id,"revision":rev+1,"visibility":"private","chunks":chunks,"contentHash":data.digest}),
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
    let input: SourceDecision = view(v);
    if input.approve != Some(true) {
        return Err(bad("Explicit decision required"));
    }
    let archived = input.archived.ok_or(bad("Archived must be boolean"))?;
    let rev = input.revision.ok_or(bad("Revision required"))?;
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
    veyndo_core::outbox::publish_on(
        &mut **tx,
        t,
        kind,
        &json!({"documentId":id,"decision":data}),
    )
    .await?;
    Ok(())
}

/// Stores a validated source once; identical content for the same product returns the existing source.
pub async fn save(
    a: &State,
    h: &RequestContext,
    t: &str,
    data: &Source,
    source: &str,
) -> Result<Value> {
    let (title, text, digest) = (&data.title, &data.content, &data.digest);
    let product = data.product_id.as_deref();
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
    sqlx::query("INSERT INTO knowledge_documents(tenant,id,product_id,title,content_hash,content,source_type,kind,locale,translations) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(t).bind(&id).bind(product).bind(title).bind(digest).bind(text).bind(source).bind(&data.kind).bind(&data.locale).bind(&data.translations).execute(&mut *tx).await?;
    let chunks = chunks(&mut tx, t, &id, data).await?;
    sync_document(&mut tx, t, &id, product, title, digest).await?;
    veyndo_core::outbox::publish_on(
        &mut *tx,
        t,
        "knowledge.document.ingested",
        &json!({"documentId":id,"productId":product,"sourceType":source}),
    )
    .await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"contentHash":digest,"revision":1,"visibility":"private","chunks":chunks,"sourceType":source}),
    )
}

/// One knowledge source as the source list shows it; columns checked at compile time.
struct ListedSource {
    id: String,
    product_id: Option<String>,
    title: String,
    source_type: String,
    visibility: String,
    revision: i64,
    content_hash: String,
}

/// The most recent sources of a shop.
pub async fn list(a: &State, t: &str) -> Result<Value> {
    let rows = sqlx::query_as!(
        ListedSource,
        "SELECT id,product_id,title,source_type,visibility,revision,content_hash FROM knowledge_documents WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100",
        t
    )
    .fetch_all(&a.db)
    .await?;
    Ok(
        json!({"elements":rows.iter().map(|r|json!({"id":r.id,"productId":r.product_id,"title":r.title,"sourceType":r.source_type,"visibility":r.visibility,"revision":r.revision,"contentHash":r.content_hash})).collect::<Vec<_>>() }),
    )
}

/// Changes the visibility of a source after an explicit publication decision.
pub async fn publish(a: &State, h: &RequestContext, t: &str, id: &str, v: &Value) -> Result<Value> {
    let input: SourceDecision = view(v);
    let visibility = input
        .visibility
        .as_deref()
        .filter(|v| ["private", "public"].contains(v))
        .ok_or(bad("Visibility must be private/public"))?;
    if input.approve != Some(true) {
        return Err(bad("Explicit publication decision required"));
    }
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let n=sqlx::query("UPDATE knowledge_documents SET visibility=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4 AND NOT archived AND (kind<>'guidance' OR $1='private')").bind(visibility).bind(t).bind(id).bind(input.revision).execute(&mut *tx).await?.rows_affected();
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

/// Whether any admitted chunk carries a vector of this model, so a semantic query is worth its cost.
pub async fn indexed(
    a: &State,
    t: &str,
    model: &str,
    public: bool,
    product: Option<&str>,
) -> Result<bool> {
    let indexed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM knowledge_chunks c JOIN knowledge_documents d ON d.tenant=c.tenant AND d.id=c.document_id WHERE c.tenant=$1 AND NOT d.archived AND c.embedding_model=$2 AND (NOT $3 OR d.visibility='public') AND ($4::text IS NULL OR d.product_id IS NULL OR d.product_id=$4 OR d.product_id=(SELECT parent_id FROM products WHERE tenant=$1 AND id=$4)))").bind(t).bind(model).bind(public).bind(product).fetch_one(&a.db).await?;
    Ok(indexed)
}

/// What a source search is bound to.
pub struct Search<'a> {
    pub product: Option<&'a str>,
    pub query: &'a str,
    pub public: bool,
    pub locale: &'a str,
    pub main_locale: &'a str,
    pub model: &'a str,
}

/// Bounded lexical/vector source retrieval; public questions use only explicitly published documents.
pub async fn search(a: &State, t: &str, f: Search<'_>, vector: Option<Vec<f32>>) -> Result<Value> {
    let (product, query, public, locale, main_locale, model) = (
        f.product,
        f.query,
        f.public,
        f.locale,
        f.main_locale,
        f.model,
    );
    let semantic = if let Some(vector) = vector {
        vectors::query("document", t, model, vector)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let ids = semantic
        .iter()
        .filter(|v| v["payload"]["tenant"] == t && v["payload"]["model"] == model)
        .filter_map(|v| v["payload"]["object_id"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let digests = semantic
        .iter()
        .filter(|v| v["payload"]["tenant"] == t && v["payload"]["model"] == model)
        .map(|v| {
            v["payload"]["digest"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect::<Vec<_>>();
    let rows = sqlx::query(include_str!("source_search.sql"))
        .bind(t)
        .bind(product)
        .bind(query)
        .bind(public)
        .bind(ids)
        .bind(digests)
        .bind(model)
        .bind(locale)
        .bind(main_locale)
        .fetch_all(&a.db)
        .await?;
    Ok(json!(rows.iter().map(|r|json!({"sourceId":format!("{}:{}",r.get::<String,_>("id"),r.get::<i32,_>("position")),"documentId":r.get::<String,_>("id"),"title":r.get::<String,_>("title"),"locale":r.get::<String,_>("locale"),"productId":r.get::<Option<String>,_>("product_id"),"appliesToProductId":product,"association":if r.get::<Option<String>,_>("product_id").is_none(){"shop"}else if product.is_none() || r.get::<Option<String>,_>("product_id").as_deref()==product{"product"}else{"product-family"},"sourceType":r.get::<String,_>("source_type"),"contentHash":r.get::<String,_>("content_hash"),"revision":r.get::<i64,_>("revision"),"text":r.get::<String,_>("text")})).collect::<Vec<_>>()))
}

/// Whether the source exists in this shop.
pub async fn exists(a: &State, t: &str, id: &str) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM knowledge_documents WHERE tenant=$1 AND id=$2)",
    )
    .bind(t)
    .bind(id)
    .fetch_one(&a.db)
    .await?;
    Ok(exists)
}

/// Whether the product revision and every supplied source chunk are still the ones that were read.
pub async fn sources_unchanged(
    a: &State,
    t: &str,
    id: &str,
    revision: Option<i64>,
    sources: &Value,
) -> Result<bool> {
    let unchanged: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2 AND revision=$3) AND NOT EXISTS(SELECT 1 FROM jsonb_to_recordset($4) AS s(\"sourceId\" text,\"documentId\" text,\"contentHash\" text,\"productId\" text,revision bigint,locale text,text text) WHERE NOT EXISTS(SELECT 1 FROM knowledge_documents d JOIN knowledge_chunks c ON c.tenant=d.tenant AND c.document_id=d.id WHERE d.tenant=$1 AND d.id=s.\"documentId\" AND d.visibility='public' AND NOT d.archived AND d.revision=s.revision AND d.content_hash=s.\"contentHash\" AND d.product_id IS NOT DISTINCT FROM s.\"productId\" AND d.id||':'||c.position=s.\"sourceId\" AND c.text=s.text AND c.locale=s.locale))"
    ).bind(t).bind(id).bind(revision)
        .bind(sources).fetch_one(&a.db).await?;
    Ok(unchanged)
}

/// The filter of the source inventory.
pub struct Inventory<'a> {
    pub after: &'a str,
    pub query: &'a str,
    pub archived: bool,
    pub kind: &'a str,
    pub locale: &'a str,
    pub main_locale: &'a str,
}

/// Knowledge census, one page of sources with its cursor, and recent activity.
pub async fn inventory(
    a: &State,
    t: &str,
    f: Inventory<'_>,
) -> Result<(Value, Vec<Value>, Value, Vec<Value>)> {
    let totals:Value=sqlx::query_scalar("SELECT jsonb_build_object('products',(SELECT count(*) FROM products WHERE tenant=$1),'sources',(SELECT count(*) FROM knowledge_documents WHERE tenant=$1 AND NOT archived),'published',(SELECT count(*) FROM knowledge_documents WHERE tenant=$1 AND visibility='public' AND NOT archived),'archived',(SELECT count(*) FROM knowledge_documents WHERE tenant=$1 AND archived),'chunks',(SELECT count(*) FROM knowledge_chunks c JOIN knowledge_documents d ON d.tenant=c.tenant AND d.id=c.document_id WHERE c.tenant=$1 AND NOT d.archived),'indexedChunks',(SELECT count(*) FROM knowledge_chunks c JOIN knowledge_documents d ON d.tenant=c.tenant AND d.id=c.document_id WHERE c.tenant=$1 AND NOT d.archived AND embedding IS NOT NULL),'external',(SELECT count(*) FROM app_evidence e JOIN app_packages p ON p.tenant=e.tenant AND p.id=e.app WHERE e.tenant=$1 AND p.active),'observedPairs',(SELECT count(*) FROM observed_pairs WHERE tenant=$1),'processedEvents',(SELECT count(*) FROM knowledge_receipts WHERE tenant=$1),'approvedSuggestions',(SELECT count(*) FROM knowledge_hypotheses WHERE tenant=$1 AND state='published'),'productViews',(SELECT coalesce(sum(views),0) FROM session_signals WHERE tenant=$1),'cartAdds',(SELECT coalesce(sum(cart_adds),0) FROM session_signals WHERE tenant=$1),'reviews',(SELECT count(*) FROM product_reviews WHERE tenant=$1 AND approved))").bind(t).fetch_one(&a.db).await?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT (to_jsonb(d)-'tenant'-'content'-'translations') || jsonb_build_object('title',COALESCE(d.translations->$6->>'title',d.translations->$7->>'title',d.title),'chunkCount',(SELECT count(*) FROM knowledge_chunks c WHERE c.tenant=d.tenant AND c.document_id=d.id),'indexedChunks',(SELECT count(*) FROM knowledge_chunks c WHERE c.tenant=d.tenant AND c.document_id=d.id AND embedding IS NOT NULL),'translationLocales',(SELECT coalesce(jsonb_agg(key),'[]') FROM jsonb_object_keys(d.translations) AS key)) FROM knowledge_documents d WHERE tenant=$1 AND id>$2 AND archived=$3 AND ($4='' OR title ILIKE '%' || $4 || '%' OR content ILIKE '%' || $4 || '%' OR translations::text ILIKE '%' || $4 || '%') AND ($5='' OR kind=$5) ORDER BY id LIMIT 51").bind(t).bind(f.after).bind(f.archived).bind(f.query).bind(f.kind).bind(f.locale).bind(f.main_locale).fetch_all(&a.db).await?;
    let next = if rows.len() > 50 {
        rows[49]["id"].clone()
    } else {
        Value::Null
    };
    let activity:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'kind',kind,'time',created_at,'sourceId',data->'documentId','hypothesisId',data->'hypothesisId') FROM outbox WHERE tenant=$1 AND (kind LIKE 'knowledge.%' OR kind LIKE 'intelligence.%' OR kind='merchant.change.applied') ORDER BY id DESC LIMIT 20").bind(t).fetch_all(&a.db).await?;
    Ok((totals, rows, next, activity))
}

/// Graph evidence and sources around one product.
pub struct ProductFacts {
    pub needs: Vec<Value>,
    pub complements: Vec<Value>,
    pub observed: Vec<Value>,
    pub sources: Vec<Value>,
    pub stats: Value,
}

/// Needs, complements, observed pairs, sources and review figures of one product.
pub async fn product_facts(
    a: &State,
    t: &str,
    id: &str,
    parent: Option<String>,
    locale: &str,
    main_locale: &str,
) -> Result<ProductFacts> {
    let needs:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('need',target_id,'source',data->'source','confidence',data->'confidence') FROM knowledge_relations WHERE tenant=$1 AND kind='SERVES' AND source_id=$2 ORDER BY target_id LIMIT 50").bind(t).bind(id).fetch_all(&a.db).await?;
    let complements:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',p.id,'name',p.name,'source',r.data->'source') FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=CASE WHEN r.source_id=$2 THEN r.target_id ELSE r.source_id END WHERE r.tenant=$1 AND r.kind='PAIRS_WITH' AND (r.source_id=$2 OR r.target_id=$2) ORDER BY p.id LIMIT 50").bind(t).bind(id).fetch_all(&a.db).await?;
    let observed:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',CASE WHEN left_id=$2 THEN right_id ELSE left_id END,'orders',orders,'lastEvent',last_event,'source','order-observation') FROM observed_pairs WHERE tenant=$1 AND (left_id=$2 OR right_id=$2) ORDER BY orders DESC LIMIT 50").bind(t).bind(id).fetch_all(&a.db).await?;
    let sources:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'title',COALESCE(translations->$4->>'title',translations->$5->>'title',title),'kind',kind,'visibility',visibility,'productId',product_id) FROM knowledge_documents WHERE tenant=$1 AND NOT archived AND (product_id=$2 OR product_id IS NULL OR product_id=$3) ORDER BY id LIMIT 50").bind(t).bind(id).bind(parent).bind(locale).bind(main_locale).fetch_all(&a.db).await?;
    let stats:Value=sqlx::query_scalar("SELECT jsonb_build_object('variants',(SELECT count(*) FROM products WHERE tenant=$1 AND parent_id=$2),'reviews',(SELECT count(*) FROM product_reviews WHERE tenant=$1 AND product_id=$2 AND approved),'averageRating',(SELECT coalesce(avg(rating),0) FROM product_reviews WHERE tenant=$1 AND product_id=$2 AND approved))").bind(t).bind(id).fetch_one(&a.db).await?;
    Ok(ProductFacts {
        needs,
        complements,
        observed,
        sources,
        stats,
    })
}

/// The shop's current private guidance sources in the locale, at most eight chunks.
pub async fn guidance(a: &State, t: &str, locale: &str) -> Result<Value> {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('documentId',d.id,'sourceId',d.id||':'||c.position,'title',d.title,'productId',d.product_id,'contentHash',d.content_hash,'revision',d.revision,'locale',c.locale,'text',c.text) FROM knowledge_documents d JOIN knowledge_chunks c ON c.tenant=d.tenant AND c.document_id=d.id WHERE d.tenant=$1 AND d.kind='guidance' AND d.visibility='private' AND NOT d.archived AND d.product_id IS NULL AND c.locale=CASE WHEN jsonb_typeof(d.translations->$2->'content')='string' THEN $2 ELSE d.locale END ORDER BY d.id,c.position LIMIT 8").bind(t).bind(locale).fetch_all(&a.db).await?;
    Ok(json!(rows))
}
