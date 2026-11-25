//! Storefront experience, policies and knowledge documents of a sandbox shop: copying, locked snapshot reads and release writes, inside the caller's transaction.
use serde_json::Value;
use veyndo_catalog::store::products::ProductRow;
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;

/// The storefront experience document, locked.
pub async fn experience(c: &mut sqlx::PgConnection, t: &str) -> Result<Option<Value>> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM experiences WHERE tenant=$1 FOR UPDATE", t)
            .fetch_optional(&mut *c)
            .await?,
    )
}

/// Copies the live storefront experience into the sandbox.
pub async fn copy_experience(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO experiences(tenant,data) SELECT $2,data FROM experiences WHERE tenant=$1",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// The sandbox's discovery and comparison policies.
pub async fn insert_policies(c: &mut sqlx::PgConnection, id: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO policy(tenant,variant) VALUES($1,'discovery'),($1,'comparison')",
        id
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Replaces the storefront experience document at the next revision.
pub async fn replace_experience(c: &mut sqlx::PgConnection, t: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE experiences SET data=$1,revision=revision+1 WHERE tenant=$2",
        data,
        t
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Copies the live knowledge sources and their chunks into a sandbox.
pub async fn copy_sources(c: &mut sqlx::PgConnection, live: &str, stage: &str) -> Result<()> {
    sqlx::query!("INSERT INTO knowledge_documents SELECT (jsonb_populate_record(NULL::knowledge_documents,to_jsonb(d)||jsonb_build_object('tenant',$2::text))).* FROM knowledge_documents d WHERE tenant=$1", live, stage).execute(&mut *c).await?;
    sqlx::query!("INSERT INTO knowledge_chunks SELECT (jsonb_populate_record(NULL::knowledge_chunks,to_jsonb(c)||jsonb_build_object('tenant',$2::text))).* FROM knowledge_chunks c WHERE tenant=$1", live, stage).execute(&mut *c).await?;
    Ok(())
}

/// The sandbox products whose knowledge relations a clone rebuilds: every parent and every product a source describes.
pub async fn source_products(c: &mut sqlx::PgConnection, stage: &str) -> Result<Vec<Product>> {
    Ok(sqlx::query_as!(ProductRow, "SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND (parent_id IS NULL OR id IN (SELECT product_id FROM knowledge_documents WHERE tenant=$1))", stage)
        .fetch_all(&mut *c)
        .await?
        .into_iter()
        .map(Product::from)
        .collect())
}

/// The identity of one knowledge source.
pub struct SourceIdentity {
    pub id: String,
    pub product_id: Option<String>,
    pub title: String,
    pub content_hash: String,
}

/// Every knowledge source of a shop.
pub async fn sources(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<SourceIdentity>> {
    Ok(sqlx::query_as!(
        SourceIdentity,
        "SELECT id,product_id,title,content_hash FROM knowledge_documents WHERE tenant=$1",
        t
    )
    .fetch_all(&mut *c)
    .await?)
}

/// Every knowledge source as a document without tenant, revision and creation time, locked.
pub async fn source_units(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(d)-'tenant'-'revision'-'created_at' AS "data!" FROM knowledge_documents d WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut *c).await?)
}

/// The columns of a reviewed knowledge source a release writes into the live shop.
pub struct PublishedSource<'a> {
    pub product_id: Option<&'a str>,
    pub title: &'a str,
    pub content_hash: &'a str,
    pub content: Option<&'a str>,
    pub visibility: Option<&'a str>,
    pub source_type: Option<&'a str>,
    pub kind: Option<&'a str>,
    pub locale: Option<&'a str>,
    pub translations: &'a Value,
    pub archived: bool,
}

/// Writes a reviewed source into the live shop and replaces its chunks with the sandbox's.
pub async fn publish_source(
    c: &mut sqlx::PgConnection,
    (live, stage, id): (&str, &str, &str),
    s: &PublishedSource<'_>,
) -> Result<()> {
    sqlx::query!("INSERT INTO knowledge_documents(tenant,id,product_id,title,content_hash,content,visibility,source_type,kind,locale,translations,archived) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(tenant,id) DO UPDATE SET product_id=EXCLUDED.product_id,title=EXCLUDED.title,content_hash=EXCLUDED.content_hash,content=EXCLUDED.content,visibility=EXCLUDED.visibility,source_type=EXCLUDED.source_type,kind=EXCLUDED.kind,locale=EXCLUDED.locale,translations=EXCLUDED.translations,archived=EXCLUDED.archived,revision=knowledge_documents.revision+1", live, id, s.product_id, s.title, s.content_hash, s.content, s.visibility, s.source_type, s.kind, s.locale, s.translations, s.archived).execute(&mut *c).await?;
    sqlx::query!(
        "DELETE FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2",
        live,
        id
    )
    .execute(&mut *c)
    .await?;
    sqlx::query!("INSERT INTO knowledge_chunks SELECT (jsonb_populate_record(NULL::knowledge_chunks,to_jsonb(c)||jsonb_build_object('tenant',$3::text))).* FROM knowledge_chunks c WHERE tenant=$1 AND document_id=$2", stage, id, live).execute(&mut *c).await?;
    Ok(())
}
