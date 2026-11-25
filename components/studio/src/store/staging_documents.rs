//! Knowledge documents/chunks clone and publish with their source provenance; publication visibility is a reviewed unit.
use crate::domain::views::Identified;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use veyndo_catalog::store::products;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, StatusCode};
use veyndo_intelligence::knowledge::{sync_document, sync_product};

/// The release transaction of a staging shop.
pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// Copies the live knowledge sources and their chunks into a staging shop and rebuilds its relations.
pub async fn clone_sources(tx: &mut Tx<'_>, live: &str, stage: &str) -> Result<()> {
    sqlx::query!("INSERT INTO knowledge_documents SELECT (jsonb_populate_record(NULL::knowledge_documents,to_jsonb(d)||jsonb_build_object('tenant',$2::text))).* FROM knowledge_documents d WHERE tenant=$1", live, stage).execute(&mut **tx).await?;
    sqlx::query!("INSERT INTO knowledge_chunks SELECT (jsonb_populate_record(NULL::knowledge_chunks,to_jsonb(c)||jsonb_build_object('tenant',$2::text))).* FROM knowledge_chunks c WHERE tenant=$1", live, stage).execute(&mut **tx).await?;
    // dynamic SQL: the whole row is decoded by the catalog's shared `product(&PgRow)` mapper.
    let products = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND (parent_id IS NULL OR id IN (SELECT product_id FROM knowledge_documents WHERE tenant=$1))")
        .bind(stage)
        .fetch_all(&mut **tx)
        .await?;
    for r in products {
        sync_product(tx, stage, &json!(products::product(&r))).await?;
    }
    let docs = sqlx::query!(
        "SELECT id,product_id,title,content_hash FROM knowledge_documents WHERE tenant=$1",
        stage
    )
    .fetch_all(&mut **tx)
    .await?;
    for r in docs {
        sync_document(
            tx,
            stage,
            &r.id,
            r.product_id.as_deref(),
            &r.title,
            &r.content_hash,
        )
        .await?;
    }
    Ok(())
}
/// Adds every knowledge source as a release unit, locked for the comparison.
pub async fn snapshot(
    tx: &mut Tx<'_>,
    tenant: &str,
    result: &mut Map<String, Value>,
) -> Result<()> {
    let rows=sqlx::query_scalar!(r#"SELECT to_jsonb(d)-'tenant'-'revision'-'created_at' AS "data!" FROM knowledge_documents d WHERE tenant=$1 ORDER BY id FOR UPDATE"#, tenant).fetch_all(&mut **tx).await?;
    for data in rows {
        result.insert(
            format!("document:{}", view::<Identified>(&data).id.unwrap()),
            data,
        );
    }
    Ok(())
}
/// A staged knowledge source as its release unit stores it, in the table's column names.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StagedSource {
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    title: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content_hash: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content: Option<String>,
    #[serde(deserialize_with = "lenient")]
    visibility: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_type: Option<String>,
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    translations: Value,
    #[serde(deserialize_with = "lenient")]
    archived: Option<bool>,
}

/// Publishes one reviewed source with its chunks and relations into the live shop.
pub async fn publish(tx: &mut Tx<'_>, live: &str, stage: &str, id: &str, v: &Value) -> Result<()> {
    let source: StagedSource = view(v);
    let incomplete = || {
        Error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Staged knowledge source is incomplete".into(),
        )
    };
    let title = source.title.as_deref().ok_or_else(incomplete)?;
    let content_hash = source.content_hash.as_deref().ok_or_else(incomplete)?;
    sqlx::query!("INSERT INTO knowledge_documents(tenant,id,product_id,title,content_hash,content,visibility,source_type,kind,locale,translations,archived) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(tenant,id) DO UPDATE SET product_id=EXCLUDED.product_id,title=EXCLUDED.title,content_hash=EXCLUDED.content_hash,content=EXCLUDED.content,visibility=EXCLUDED.visibility,source_type=EXCLUDED.source_type,kind=EXCLUDED.kind,locale=EXCLUDED.locale,translations=EXCLUDED.translations,archived=EXCLUDED.archived,revision=knowledge_documents.revision+1", live, id, source.product_id.as_deref(), title, content_hash, source.content.as_deref(), source.visibility.as_deref(), source.source_type.as_deref(), source.kind.as_deref(), source.locale.as_deref(), &source.translations, source.archived.unwrap_or(false)).execute(&mut **tx).await?;
    sqlx::query!(
        "DELETE FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2",
        live,
        id
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!("INSERT INTO knowledge_chunks SELECT (jsonb_populate_record(NULL::knowledge_chunks,to_jsonb(c)||jsonb_build_object('tenant',$3::text))).* FROM knowledge_chunks c WHERE tenant=$1 AND document_id=$2", stage, id, live).execute(&mut **tx).await?;
    if let Some(product) = source.product_id.as_deref() {
        let published = super::products::get(tx, live, product).await?;
        sync_product(tx, live, &json!(published)).await?;
    }
    sync_document(
        tx,
        live,
        id,
        source.product_id.as_deref(),
        title,
        content_hash,
    )
    .await?;
    Ok(())
}
