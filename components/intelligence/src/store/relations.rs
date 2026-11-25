//! Knowledge relations in storage: idempotent edges, their removal and the graph read model. Columns checked at compile time.
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};

/// Writes one relation; an existing edge keeps its identity and takes the new data.
pub async fn edge(
    e: impl PgExecutor<'_>,
    tenant: &str,
    kind: &str,
    left: &str,
    right: &str,
    data: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!("INSERT INTO knowledge_relations(tenant,kind,source_id,target_id,data) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,kind,source_id,target_id) DO UPDATE SET data=EXCLUDED.data",
        tenant, kind, left, right, data).execute(e).await?;
    Ok(())
}

/// Locks the canonical product row, as order observation transactions do.
pub async fn lock_product(
    conn: &mut PgConnection,
    tenant: &str,
    id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT id FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Whether the shop runs the fashion demo catalogue.
pub async fn fashion_demo(e: impl PgExecutor<'_>, tenant: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id='coat' AND extra->'demo'->>'catalogVersion'='nord-atelier-v1') AS "fashion!""#, tenant).fetch_one(e).await
}

/// The relation graph of a shop: curated needs and pairs, observed pairs and product documents.
pub struct Graph {
    pub needs: Vec<Value>,
    pub pairs: Vec<Value>,
    pub observed: Vec<Value>,
    pub documents: Vec<Value>,
}

/// Reads the relation graph of a shop.
pub async fn graph(
    db: &veyndo_runtime::scoped_pool::ScopedPool,
    tenant: &str,
) -> Result<Graph, sqlx::Error> {
    let needs=sqlx::query_scalar!(r#"SELECT jsonb_build_object('product_id',r.source_id,'need',r.target_id,'source',r.data->'source','confidence',r.data->'confidence') AS "need!" FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=r.source_id WHERE r.tenant=$1 AND r.kind='SERVES' ORDER BY r.source_id,r.target_id LIMIT 48"#, tenant).fetch_all(db).await?;
    let pairs=sqlx::query_scalar!(r#"SELECT jsonb_build_object('left',r.source_id,'right',r.target_id,'source',r.data->'source') AS "pair!" FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=r.source_id JOIN products q ON q.tenant=r.tenant AND q.id=r.target_id WHERE r.tenant=$1 AND r.kind='PAIRS_WITH' ORDER BY r.source_id,r.target_id LIMIT 48"#, tenant).fetch_all(db).await?;
    let observed=sqlx::query_scalar!(r#"SELECT jsonb_build_object('left',r.source_id,'right',r.target_id,'orders',r.data->'orders','source',r.data->'source','lastEvent',r.data->'event_id') AS "pair!" FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=r.source_id JOIN products q ON q.tenant=r.tenant AND q.id=r.target_id WHERE r.tenant=$1 AND r.kind='CO_PURCHASED' ORDER BY (r.data->>'orders')::bigint DESC,r.source_id,r.target_id LIMIT 24"#, tenant).fetch_all(db).await?;
    let documents=sqlx::query_scalar!(r#"SELECT jsonb_build_object('product_id',r.source_id,'document_id',d.id,'title',d.title,'source','merchant-document') AS "document!" FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=r.source_id JOIN knowledge_documents d ON d.tenant=r.tenant AND d.id=r.target_id AND d.visibility='public' AND NOT d.archived WHERE r.tenant=$1 AND r.kind='HAS_DOCUMENT' ORDER BY r.source_id,d.id LIMIT 100"#, tenant).fetch_all(db).await?;
    Ok(Graph {
        needs,
        pairs,
        observed,
        documents,
    })
}

/// Removes the product link of a document.
pub async fn unlink_document(
    e: impl PgExecutor<'_>,
    tenant: &str,
    id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM knowledge_relations WHERE tenant=$1 AND kind='HAS_DOCUMENT' AND target_id=$2",
        tenant,
        id
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Removes the product references of an integration source (`id` empty: of every source of the app).
pub async fn unlink_integration(
    e: impl PgExecutor<'_>,
    tenant: &str,
    app: &str,
    id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM knowledge_relations WHERE tenant=$1 AND kind='REFERENCES_PRODUCT' AND data->>'app'=$2 AND ($3='' OR data->>'sourceId'=$3)", tenant, app, id).execute(e).await?;
    Ok(())
}
