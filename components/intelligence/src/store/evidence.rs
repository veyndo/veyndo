//! Source-bound claim lifecycle on the knowledge relation ledger; no model text becomes a confirmed fact automatically.
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, State, StatusCode, bad, conflict, uid};
use veyndo_kernel::verified_kernel;

/// The fact node kinds a claim may describe.
pub const NODE_TYPES: &[&str] = &[
    "product",
    "variant",
    "material",
    "property",
    "intent",
    "problem",
    "occasion",
    "audience",
    "claim",
    "return_reason",
    "supplier",
    "policy",
    "document",
    "support",
];
/// The claims of a product; `public` keeps only confirmed ones on current public sources.
pub async fn claims(a: &State, t: &str, id: &str, public: bool) -> Result<Value> {
    let rows = claim_rows(a, t, id, public).await?;
    Ok(json!({"productId":id,"claims":rows,"nodeTypes":NODE_TYPES,"modelWeightsUpdated":false}))
}

/// The claim records of a product, newest first (at most 100).
pub async fn claim_rows(a: &State, t: &str, id: &str, public: bool) -> Result<Vec<Value>> {
    Ok(
        sqlx::query_file_scalar!("src/store/evidence_read.sql", t, id, public)
            .fetch_all(&a.db)
            .await?,
    )
}
/// The current confirmed public claims of several products in one language, in one bounded read.
pub async fn claim_facts(a: &State, t: &str, ids: &[String], locale: &str) -> Result<Vec<Value>> {
    let rows = sqlx::query_file_scalar!("src/store/claim_batch.sql", t, ids, locale)
        .fetch_all(&a.db)
        .await?;
    Ok(rows)
}
/// A proposed claim: the product, the source and its hash, the claim text and the quote that supports it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ClaimProposal {
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    text: Option<String>,
    #[serde(deserialize_with = "lenient")]
    quote: Option<String>,
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content_hash: Option<String>,
    #[serde(deserialize_with = "lenient")]
    node_type: Option<String>,
}

/// A merchant decision on a claim at the revision it was shown.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ClaimDecision {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<u64>,
}

/// The source a stored claim was extracted from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ClaimSource {
    #[serde(deserialize_with = "lenient")]
    source_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content_hash: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_revision: Option<i64>,
}

/// Records a proposed claim whose quote is present in the current source; a merchant decides on it later.
pub async fn propose_in(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    actor: &str,
    v: &Value,
    main_locale: &str,
    locales: &[String],
) -> Result<Value> {
    let confidence = confidence(v)?;
    let input: ClaimProposal = view(v);
    let product = input.product_id.as_deref().ok_or(bad("Product required"))?;
    let source = input.source_id.as_deref().ok_or(bad("Source required"))?;
    let text = input
        .text
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 1200)
        .ok_or(bad("Claim must be 1..1200 bytes"))?;
    let quote = input
        .quote
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 2400)
        .ok_or(bad("Source quote required"))?;
    let locale = input.locale.as_deref().unwrap_or(main_locale);
    if !locales.iter().any(|l| l == locale) {
        return Err(bad("Claim language must be enabled"));
    }

    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        product
    )
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(Error(StatusCode::NOT_FOUND, "Product not found".into()));
    }
    let row = sqlx::query!("SELECT content_hash,revision FROM knowledge_documents WHERE tenant=$1 AND id=$2 AND NOT archived AND (product_id IS NULL OR product_id=$3) FOR SHARE",
        t, source, product).fetch_optional(&mut **tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Source not found for product".into()))?;
    if input.content_hash.as_deref() != Some(row.content_hash.as_str()) {
        return Err(conflict("Source changed; retrieve current evidence"));
    }
    let sourced = sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2 AND locale=$3 AND position($4 in text)>0) AS "sourced!""#,
        t, source, locale, quote).fetch_one(&mut **tx).await?;
    if !sourced {
        return Err(bad("Quote is absent from this source/language"));
    }
    let id = uid();
    let node_type = input.node_type.as_deref().unwrap_or("claim");
    if !NODE_TYPES.contains(&node_type) {
        return Err(bad("Unknown fact node type"));
    }
    let data = json!({"text":text,"quote":quote,"locale":locale,"sourceId":source,"sourceRevision":row.revision,"contentHash":input.content_hash,"sourceType":"document","truthVerified":false,"nodeType":node_type});
    sqlx::query!("INSERT INTO knowledge_relations(tenant,kind,source_id,target_id,target_kind,state,data,actor,confidence) VALUES($1,'CLAIMS',$2,$3,$7,'proposed',$4,$5,$6::float8)",
        t, product, id, data, actor, confidence, node_type).execute(&mut **tx).await?;

    Ok(json!({"id":id,"revision":1,"state":"proposed","approvalRequired":true}))
}
/// Omitted confidence uses the documented candidate prior; malformed supplied values never become an invented score.
fn confidence(v: &Value) -> Result<f64> {
    match v.get("confidence") {
        None => Ok(0.5),
        Some(value) => value
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
            .ok_or(bad(
                "Claim confidence must be a finite number between 0 and 1",
            )),
    }
}
/// A merchant decision on a claim at the expected revision; confirming needs the unchanged source.
pub async fn decide(a: &State, t: &str, actor: Option<&str>, v: &Value) -> Result<Value> {
    let input: ClaimDecision = view(v);
    if input.approve != Some(true) {
        return Err(bad("Explicit approve=true required"));
    }
    let state = input
        .state
        .as_deref()
        .filter(|s| ["evidenced", "confirmed", "rejected"].contains(s))
        .ok_or(bad("Invalid evidence state"))?;
    let id = input.id.as_deref().ok_or(bad("Claim ID required"))?;
    let mut tx = a.db.begin().await?;
    let row = sqlx::query!("SELECT source_id,data,revision FROM knowledge_relations WHERE tenant=$1 AND kind='CLAIMS' AND target_id=$2 FOR UPDATE",
        t, id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Claim not found".into()))?;
    let revision = row.revision;
    if !verified_kernel::revision_admissible(revision as u64, input.revision.unwrap_or(0)) {
        return Err(conflict("Claim revision changed"));
    }
    let data: ClaimSource = view(&row.data);
    let fresh = sqlx::query_scalar!(r#"SELECT true AS "fresh!" FROM knowledge_documents WHERE tenant=$1 AND id=$2 AND content_hash=$3 AND revision=$4 AND NOT archived FOR SHARE"#,
        t, data.source_id, data.content_hash, data.source_revision).fetch_optional(&mut *tx).await?.unwrap_or(false);
    if !fresh && state != "rejected" {
        return Err(conflict("Source changed or archived; extract again"));
    }
    sqlx::query!("UPDATE knowledge_relations SET state=$1,actor=$2 WHERE tenant=$3 AND kind='CLAIMS' AND target_id=$4",
        state, actor.unwrap_or("integration"), t, id).execute(&mut *tx).await?;
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        t,
        "intelligence.claim.reviewed",
        &json!({"claimId":id,"state":state,"actor":actor}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"state":state,"revision":revision+1}))
}

#[cfg(test)]
mod tests {
    use super::confidence;
    use serde_json::json;
    #[test]
    fn candidate_confidence_is_explicitly_validated() {
        assert_eq!(confidence(&json!({})).unwrap(), 0.5);
        for n in [0.0, 0.9, 1.0] {
            assert_eq!(confidence(&json!({"confidence":n})).unwrap(), n);
        }
        for value in [
            json!(-0.1),
            json!(1.1),
            json!("0.9"),
            json!(null),
            json!(true),
            json!({}),
            json!([]),
        ] {
            assert!(confidence(&json!({"confidence":value})).is_err());
        }
    }
}
