//! The vector index in storage: the leased queue, the current source of a queued object, acknowledgment or retry, and the index status. Checked against the schema at compile time.
use sqlx::PgExecutor;

/// A queued vector index change under a lease.
pub struct Claimed {
    pub tenant: String,
    pub kind: String,
    pub object_id: String,
    pub revision: i64,
}

/// Leases up to 16 due changes of active shops.
pub async fn claim(e: impl PgExecutor<'_>, lease: &str) -> Result<Vec<Claimed>, sqlx::Error> {
    sqlx::query_file_as!(Claimed, "src/store/vector_claim.sql", lease)
        .fetch_all(e)
        .await
}

/// The current embedding of a queued object.
pub struct Source {
    pub embedding: Vec<f32>,
    pub model: String,
    pub digest: String,
}

/// The current embedding of a product, or of a document chunk (`id` = document ID and position).
pub async fn source(
    e: impl PgExecutor<'_>,
    tenant: &str,
    kind: &str,
    id: &str,
) -> Result<Option<Source>, sqlx::Error> {
    if kind == "product" {
        sqlx::query_as!(Source, r#"SELECT embedding,embedding_model AS model,content_hash AS digest FROM semantic_products WHERE tenant=$1 AND product_id=$2"#,
            tenant, id).fetch_optional(e).await
    } else {
        sqlx::query_as!(Source, r#"SELECT c.embedding AS "embedding!",c.embedding_model AS "model!",d.content_hash AS digest FROM knowledge_chunks c JOIN knowledge_documents d ON d.tenant=c.tenant AND d.id=c.document_id WHERE c.tenant=$1 AND c.document_id||':'||c.position=$2 AND c.embedding IS NOT NULL"#,
            tenant, id).fetch_optional(e).await
    }
}

/// Removes a published change that still holds the lease.
pub async fn acknowledge(
    e: impl PgExecutor<'_>,
    c: &Claimed,
    lease: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM vector_index_queue WHERE tenant=$1 AND kind=$2 AND object_id=$3 AND revision=$4 AND lease=$5",
        c.tenant, c.kind, c.object_id, c.revision, lease).execute(e).await?;
    Ok(())
}

/// Releases a failed change for a later attempt with exponential backoff (at most 300 seconds).
pub async fn retry(e: impl PgExecutor<'_>, c: &Claimed, lease: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE vector_index_queue SET attempts=attempts+1,lease=NULL,lease_until=NULL,error_code='vector_publication_unavailable',available_at=now()+least(300,power(2,least(attempts+1,8))::int)*interval '1 second' WHERE tenant=$1 AND kind=$2 AND object_id=$3 AND revision=$4 AND lease=$5",
        c.tenant, c.kind, c.object_id, c.revision, lease).execute(e).await?;
    Ok(())
}

/// Whether the shop has products indexed with the embedding model.
pub async fn indexed(
    e: impl PgExecutor<'_>,
    tenant: &str,
    model: &str,
) -> veyndo_core::Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM semantic_products WHERE tenant=$1 AND embedding_model=$2) AS "indexed!""#,
        tenant,
        model
    )
    .fetch_one(e)
    .await?)
}

/// One indexed product.
pub struct IndexedProduct {
    pub product_id: String,
    pub embedding_model: String,
    pub content_hash: String,
    pub vector_digest: String,
}

/// The shop's first 200 indexed products.
pub async fn indexed_products(
    e: impl PgExecutor<'_>,
    tenant: &str,
) -> veyndo_core::Result<Vec<IndexedProduct>> {
    Ok(sqlx::query_as!(IndexedProduct, r#"SELECT product_id,embedding_model,content_hash,vector_digest AS "vector_digest!" FROM semantic_products WHERE tenant=$1 ORDER BY product_id LIMIT 200"#,
        tenant).fetch_all(e).await?)
}

/// The shop's index progress.
pub struct IndexStatus {
    pub indexed_products: i64,
    pub pending_jobs: i64,
}

/// The shop's index progress, if recorded.
pub async fn index_status(
    e: impl PgExecutor<'_>,
    tenant: &str,
) -> veyndo_core::Result<Option<IndexStatus>> {
    Ok(sqlx::query_as!(
        IndexStatus,
        "SELECT indexed_products,pending_jobs FROM knowledge_index_status WHERE tenant=$1",
        tenant
    )
    .fetch_optional(e)
    .await?)
}

/// The shop's index error counts by code (at most 16).
pub async fn index_errors(
    e: impl PgExecutor<'_>,
    tenant: &str,
) -> veyndo_core::Result<Vec<serde_json::Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('code',e.key,'count',e.value) AS "error!" FROM knowledge_index_status s CROSS JOIN LATERAL jsonb_each(s.errors) e WHERE tenant=$1 ORDER BY e.key LIMIT 16"#,
        tenant).fetch_all(e).await?)
}
