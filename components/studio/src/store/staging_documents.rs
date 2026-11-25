//! Knowledge documents/chunks clone and publish with their source provenance; publication visibility is a reviewed unit.
use crate::domain::views::Identified;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, StatusCode};
use veyndo_intelligence::knowledge::{sync_document, sync_product};
use veyndo_intelligence::store::staging as knowledge;

/// The release transaction of a staging shop.
pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// Copies the live knowledge sources and their chunks into a staging shop and rebuilds its relations.
pub async fn clone_sources(tx: &mut Tx<'_>, live: &str, stage: &str) -> Result<()> {
    knowledge::copy_sources(tx, live, stage).await?;
    let products = knowledge::source_products(tx, stage).await?;
    for product in products {
        sync_product(tx, stage, &json!(product)).await?;
    }
    let docs = knowledge::sources(tx, stage).await?;
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
    let rows = knowledge::source_units(tx, tenant).await?;
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
    knowledge::publish_source(
        tx,
        (live, stage, id),
        &knowledge::PublishedSource {
            product_id: source.product_id.as_deref(),
            title,
            content_hash,
            content: source.content.as_deref(),
            visibility: source.visibility.as_deref(),
            source_type: source.source_type.as_deref(),
            kind: source.kind.as_deref(),
            locale: source.locale.as_deref(),
            translations: &source.translations,
            archived: source.archived.unwrap_or(false),
        },
    )
    .await?;
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
