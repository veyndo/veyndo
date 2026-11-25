//! Private provenance-bearing app sources for merchant retrieval; never public PDP answers.
use crate::store::locks;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, State, bad, hash};
use veyndo_intelligence::knowledge::sync_integration;

/// The best matching private sources of active apps for a merchant query.
pub async fn private_evidence(a: &State, t: &str, query: &str) -> Result<Value> {
    let rows = sqlx::query!(
        "SELECT e.app,e.source_id,e.title,e.body,e.source_url,e.metadata,e.digest FROM app_evidence e JOIN app_packages p ON p.tenant=e.tenant AND p.id=e.app AND p.active WHERE e.tenant=$1 ORDER BY ts_rank_cd(to_tsvector('simple',e.title||' '||e.body),plainto_tsquery('simple',$2)) DESC,e.updated_at DESC LIMIT 12",
        t,
        query
    )
    .fetch_all(&a.db)
    .await?;
    Ok(json!(rows.into_iter().map(|r|json!({"app":r.app,"sourceId":r.source_id,"title":r.title,"text":r.body.chars().take(2500).collect::<String>(),"sourceUrl":r.source_url,"metadata":r.metadata,"digest":r.digest,"visibility":"merchant-private","trustedInstructions":false})).collect::<Vec<_>>()))
}
/// The private sources of active apps and the products they relate to.
pub async fn private_graph(a: &State, t: &str) -> Result<Value> {
    let active: Vec<String> =
        sqlx::query_scalar!("SELECT id FROM app_packages WHERE tenant=$1 AND active", t)
            .fetch_all(&a.db)
            .await?;
    let sources: Vec<Value> = sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('app',app,'sourceId',source_id,'title',title,'kind',kind) AS "source!" FROM app_evidence WHERE tenant=$1 AND app=ANY($2) ORDER BY app,source_id LIMIT 100"#,
        t,
        &active
    )
    .fetch_all(&a.db)
    .await?;
    let products: Vec<Value> = sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('app',r.data->>'app','sourceId',r.data->>'sourceId','productId',p.id) AS "relation!" FROM knowledge_relations r JOIN products p ON p.tenant=r.tenant AND p.id=r.target_id JOIN app_evidence e ON e.tenant=r.tenant AND e.app=r.data->>'app' AND e.source_id=r.data->>'sourceId' WHERE r.tenant=$1 AND r.kind='REFERENCES_PRODUCT' AND e.app=ANY($2) ORDER BY e.app,e.source_id,p.id LIMIT 100"#,
        t,
        &active
    )
    .fetch_all(&a.db)
    .await?;
    Ok(json!({"sources":sources,"productRelations":products,"visibility":"merchant-private"}))
}

/// The JSON fields `ingest` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct IngestInput {
    #[serde(deserialize_with = "lenient")]
    cursor: Option<i64>,
    sources: Value,
}

/// One exported source of an app, typed as the import reads it; the original is mirrored as sent.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Source {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    deleted: Value,
    #[serde(deserialize_with = "lenient")]
    title: Option<String>,
    #[serde(deserialize_with = "lenient")]
    text: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_url: Option<String>,
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    metadata: Value,
}

/// Stores one export page of an app's private sources at its cursor, mirrors them into the knowledge graph and announces new ones.
pub async fn ingest(a: &State, t: &str, app: &str, previous: i64, v: &Value) -> Result<()> {
    let input: IngestInput = view(v);
    let cursor = input
        .cursor
        .filter(|n| *n >= previous)
        .ok_or(bad("Invalid export cursor"))?;
    let sources = input
        .sources
        .as_array()
        .filter(|s| s.len() <= 10)
        .ok_or(bad("Invalid app sources"))?;
    if sources.is_empty() {
        return Ok(());
    }
    let mut tx = a.db.begin().await?;
    // AGE MERGE has no unique constraint: serialize shared Shop-node writes across apps.
    locks::advisory(&mut tx, t, 39).await?;
    let current = sqlx::query_scalar!(
        r#"SELECT coalesce((SELECT cursor FROM app_export_cursors WHERE tenant=$1 AND app=$2),0) AS "cursor!""#,
        t,
        app
    )
    .fetch_one(&mut *tx)
    .await?;
    if current != previous {
        return Ok(());
    }
    for s in sources {
        let source: Source = view(s);
        let id = source
            .id
            .as_deref()
            .filter(|s| !s.is_empty() && s.len() <= 150)
            .ok_or(bad("Invalid source ID"))?;
        if source.deleted == true {
            // The connector's existing reset tombstone must survive a lost action reply.
            // It is still scoped to this admitted package and tenant.
            sqlx::query!(
                "DELETE FROM app_evidence WHERE tenant=$1 AND app=$2 AND ($3::text='reset' OR source_id=$3)",
                t,
                app,
                id
            )
            .execute(&mut *tx)
            .await?;
            sync_integration(
                &mut tx,
                t,
                app,
                if id == "reset" { "" } else { id },
                s,
                true,
            )
            .await?;
            continue;
        }
        let title = source
            .title
            .as_deref()
            .filter(|s| s.len() <= 300)
            .ok_or(bad("Invalid source title"))?;
        let text = source
            .text
            .as_deref()
            .filter(|s| s.len() <= 12000)
            .ok_or(bad("Invalid source text"))?;
        let url = source
            .source_url
            .as_deref()
            .filter(|s| s.starts_with("https://") && s.len() <= 1500)
            .ok_or(bad("Invalid source URL"))?;
        let kind = source
            .kind
            .as_deref()
            .filter(|s| s.len() <= 50)
            .ok_or(bad("Invalid source kind"))?;
        let digest = hash(&s.to_string());
        let n = sqlx::query!(
            "INSERT INTO app_evidence(tenant,app,source_id,kind,title,body,source_url,metadata,digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(tenant,app,source_id) DO UPDATE SET title=EXCLUDED.title,body=EXCLUDED.body,source_url=EXCLUDED.source_url,metadata=EXCLUDED.metadata,digest=EXCLUDED.digest,updated_at=now() WHERE app_evidence.digest<>EXCLUDED.digest",
            t,
            app,
            id,
            kind,
            title,
            text,
            url,
            &source.metadata,
            digest
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n > 0 {
            sync_integration(&mut tx, t, app, id, s, false).await?;
            veyndo_core::store::outbox::publish_on(&mut *tx, t, &format!("app.{app}.source_imported"), &json!({"sourceId":id,"app":app,"sourceKind":kind,"title":title,"metadata":source.metadata})).await?;
        }
    }
    sqlx::query!(
        "INSERT INTO app_export_cursors(tenant,app,cursor) VALUES($1,$2,$3) ON CONFLICT(tenant,app) DO UPDATE SET cursor=greatest(app_export_cursors.cursor,EXCLUDED.cursor)",
        t,
        app,
        cursor
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
/// Removes every private source of an app and resets its export cursor.
pub async fn purge_sources(a: &State, t: &str, app: &str, cursor: i64) -> Result<()> {
    let mut tx = a.db.begin().await?;
    locks::advisory(&mut tx, t, 39).await?;
    sqlx::query!(
        "DELETE FROM app_evidence WHERE tenant=$1 AND app=$2",
        t,
        app
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO app_export_cursors(tenant,app,cursor) VALUES($1,$2,$3) ON CONFLICT(tenant,app) DO UPDATE SET cursor=EXCLUDED.cursor",
        t,
        app,
        cursor
    )
    .execute(&mut *tx)
    .await?;
    sync_integration(&mut tx, t, app, "", &json!({}), true).await?;
    tx.commit().await?;
    Ok(())
}

/// Up to 100 active packages of the configured services that may write knowledge, least recently polled first, with their export cursor.
pub async fn export_candidates(a: &State, ids: &[String]) -> Result<Vec<(String, String, i64)>> {
    let rows = sqlx::query!(
        r#"SELECT p.tenant,p.id,coalesce(c.cursor,0) AS "cursor!" FROM app_packages p LEFT JOIN app_export_cursors c ON c.tenant=p.tenant AND c.app=p.id WHERE p.active AND p.id=ANY($1) AND p.manifest->'permissions' ? 'knowledge.write' ORDER BY coalesce(c.last_poll,'epoch'),p.tenant,p.id LIMIT 100"#,
        ids
    )
    .fetch_all(&a.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.tenant, r.id, r.cursor))
        .collect())
}

/// Records that the app's exports were polled now.
pub async fn touch_export(a: &State, t: &str, app: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO app_export_cursors(tenant,app,last_poll) VALUES($1,$2,now()) ON CONFLICT(tenant,app) DO UPDATE SET last_poll=now()",
        t,
        app
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
