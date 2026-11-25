//! Keep knowledge provenance and relations in the same transaction as canonical commerce data.
use super::PgPool;
use crate::store::relations as store;
use crate::store::relations::edge;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::document::{lenient, view};
/// The JSON fields `sync_product` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SyncProductInput {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

pub async fn sync_product(
    conn: &mut PgConnection,
    tenant: &str,
    product: &Value,
) -> Result<(), sqlx::Error> {
    let input: SyncProductInput = view(product);
    // Preserve canonical-product locking used by order observation transactions.
    store::lock_product(conn, tenant, input.id.as_deref()).await
}
pub async fn seed_relations(db: &PgPool, tenant: &str) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let fashion = store::fashion_demo(&mut *tx, tenant).await?;
    let needs = if fashion {
        vec![
            ("coat", vec!["layering", "cold-weather"]),
            ("knit", vec!["layering", "everyday"]),
            ("shirt", vec!["work", "everyday"]),
            ("trousers", vec!["work"]),
            ("dress", vec!["occasion"]),
            ("blazer", vec!["work", "layering"]),
            ("tee", vec!["everyday", "layering"]),
            ("jeans", vec!["everyday"]),
            ("skirt", vec!["everyday", "work"]),
            ("sneakers", vec!["everyday"]),
            ("bag", vec!["everyday", "work"]),
            ("scarf", vec!["layering", "cold-weather"]),
        ]
    } else {
        vec![
            ("chair", vec!["reading", "work", "small-space"]),
            ("desk", vec!["work", "small-space"]),
            ("lamp", vec!["reading", "work", "warm-light"]),
            ("mug", vec!["coffee", "reading"]),
            ("notebook", vec!["notes", "work"]),
            ("shelf", vec!["storage", "work", "small-space"]),
        ]
    };
    for (id, needs) in needs {
        for need in needs {
            edge(
                &mut *tx,
                tenant,
                "SERVES",
                id,
                need,
                json!({"source":"demo-curated","confidence":1.0}),
            )
            .await?;
        }
    }
    let pairs = if fashion {
        vec![
            ("coat", "knit"),
            ("coat", "scarf"),
            ("shirt", "trousers"),
            ("blazer", "trousers"),
            ("tee", "jeans"),
            ("jeans", "sneakers"),
            ("skirt", "knit"),
            ("dress", "bag"),
        ]
    } else {
        vec![
            ("desk", "chair"),
            ("chair", "lamp"),
            ("desk", "lamp"),
            ("desk", "notebook"),
            ("desk", "shelf"),
        ]
    };
    for (left, right) in pairs {
        edge(
            &mut *tx,
            tenant,
            "PAIRS_WITH",
            left,
            right,
            json!({"source":"demo-curated","confidence":1.0}),
        )
        .await?;
    }
    tx.commit().await
}
pub async fn graph(db: &PgPool, tenant: &str) -> Result<Value, sqlx::Error> {
    let store::Graph {
        needs,
        pairs,
        observed,
        documents,
    } = store::graph(db, tenant).await?;
    Ok(
        json!({"engine":"PostgreSQL","tenant":tenant,"needs":needs,"pairs":pairs,"observedPairs":observed,"documents":documents,"facts":"SERVES/PAIRS_WITH are curated; CO_PURCHASED are order observations with evidence, not causal claims"}),
    )
}
pub async fn sync_document(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    product: Option<&str>,
    _title: &str,
    _digest: &str,
) -> Result<(), sqlx::Error> {
    store::unlink_document(&mut *conn, tenant, id).await?;
    if let Some(product) = product {
        edge(
            conn,
            tenant,
            "HAS_DOCUMENT",
            product,
            id,
            json!({"source":"merchant-document"}),
        )
        .await?;
    }
    Ok(())
}
pub async fn sync_observation(
    conn: &mut PgConnection,
    tenant: &str,
    left: &str,
    right: &str,
    orders: i64,
    event: i64,
) -> Result<(), sqlx::Error> {
    edge(
        conn,
        tenant,
        "CO_PURCHASED",
        left,
        right,
        json!({"orders":orders,"event_id":event,"source":"order-observation"}),
    )
    .await
}
/// The products an integration source names in its metadata.
#[derive(Deserialize, Default)]
#[serde(default)]
struct IntegrationSource {
    #[serde(deserialize_with = "lenient")]
    metadata: SourceMetadata,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SourceMetadata {
    #[serde(deserialize_with = "lenient")]
    product_ids: Option<Vec<Value>>,
}

pub async fn sync_integration(
    conn: &mut PgConnection,
    tenant: &str,
    app: &str,
    id: &str,
    source: &Value,
    deleted: bool,
) -> Result<(), sqlx::Error> {
    // JSON tuple identity avoids ambiguity from separators in provider IDs.
    let key = json!([app, id]).to_string();
    store::unlink_integration(&mut *conn, tenant, app, id).await?;
    if !deleted && let Some(ids) = view::<IntegrationSource>(source).metadata.product_ids {
        for product in ids.iter().filter_map(Value::as_str).take(30) {
            edge(
                &mut *conn,
                tenant,
                "REFERENCES_PRODUCT",
                &key,
                product,
                json!({"app":app,"sourceId":id}),
            )
            .await?;
        }
    }
    Ok(())
}
