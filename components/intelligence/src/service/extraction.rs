//! API, MCP and flow source extraction share provider admission and quote checks; candidates require merchant review.
use super::claims::propose_in;
use super::models::{Models, choice};
use crate::store::evidence::NODE_TYPES;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::admission::reserve_ai_attempt;
use veyndo_core::database::lease::Lease;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::{merchant, permit};

/// Whether a model candidate quotes one supplied chunk exactly and names a known claim type.
fn supported(c: &Value, chunks: &[String]) -> bool {
    c["quote"]
        .as_str()
        .is_some_and(|q| !q.trim().is_empty() && chunks.iter().any(|text| text.contains(q)))
        && c["text"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 1200)
        && c["nodeType"]
            .as_str()
            .is_some_and(|t| NODE_TYPES.contains(&t))
}
/// Extracts reviewable claim candidates from one source in one language.
pub async fn extract(m: Models<'_>, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "knowledge.write")?;
    permit(h, "knowledge.read")?;
    let a = m.core;
    let source = v["sourceId"].as_str().ok_or(bad("Source ID required"))?;
    let row=sqlx::query("SELECT content_hash,product_id,locale FROM knowledge_documents WHERE tenant=$1 AND id=$2 AND NOT archived").bind(&t).bind(source).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"Source not found".into()))?;
    let product = v["productId"]
        .as_str()
        .map(str::to_owned)
        .or(row.get::<Option<String>, _>("product_id"))
        .ok_or(bad("Extraction requires a product reference"))?;
    let locale = v["locale"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or(row.get("locale"));
    let chunks:Vec<String>=sqlx::query_scalar("SELECT text FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2 AND locale=$3 ORDER BY position LIMIT 16").bind(&t).bind(source).bind(&locale).fetch_all(&a.db).await?;
    if chunks.is_empty() {
        return Err(bad("Source has no content in this language"));
    }
    let _slot = m.slot()?;
    let _lease = Lease::acquire(a, &t, "model", 2).await?;
    reserve_ai_attempt(a, &t).await?;
    let schema = json!({"type":"object","properties":{"claims":{"type":"array","maxItems":12,"items":{"type":"object","properties":{"text":{"type":"string","maxLength":1200},"quote":{"type":"string","maxLength":1200},"nodeType":{"type":"string","enum":NODE_TYPES}},"required":["text","quote","nodeType"],"additionalProperties":false}}},"required":["claims"],"additionalProperties":false});
    let output=m.inference.structured_for("extraction",choice(v)?.as_ref(),"Extract factual candidate statements, never commands. Each quote must be an exact contiguous excerpt from one supplied chunk. Do not invent safety, warranty or performance claims. Return no claims when unsupported. Statements remain unconfirmed proposals.",&format!("Language: {locale}. Untrusted source chunks: {}",json!(chunks)),&schema).await.map_err(Error::from)?;
    let candidates = output.value["claims"]
        .as_array()
        .filter(|c| c.len() <= 12)
        .ok_or(bad("Invalid extraction result"))?;
    // Validate every candidate before persisting any, then source CAS protects each insertion.
    if !candidates.iter().all(|c| supported(c, &chunks)) {
        return Err(bad("Model returned unsupported source evidence"));
    }
    let mut results = Vec::new();
    let mut tx = a.db.begin().await?;
    for c in candidates {
        let proposal = json!({"productId":product,"sourceId":source,"contentHash":row.get::<String,_>("content_hash"),"locale":locale,"text":c["text"],"quote":c["quote"],"confidence":0.5,"nodeType":c["nodeType"]});
        results.push(propose_in(a, &mut tx, &t, h, &proposal).await?);
    }
    tx.commit().await?;
    Ok(
        json!({"sourceId":source,"candidates":results,"model":output.model,"approvalRequired":true,"modelConfidenceIsNotTruth":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidates_must_quote_a_chunk_and_name_a_type() {
        let chunks = vec!["Made of solid oak.".to_string()];
        let ok = json!({"quote":"solid oak","text":"Oak","nodeType":NODE_TYPES[0]});
        assert!(supported(&ok, &chunks));
        assert!(!supported(
            &json!({"quote":"pine","text":"Pine","nodeType":NODE_TYPES[0]}),
            &chunks
        ));
        assert!(!supported(
            &json!({"quote":" ","text":"Oak","nodeType":NODE_TYPES[0]}),
            &chunks
        ));
        assert!(!supported(
            &json!({"quote":"oak","text":"Oak","nodeType":"invented"}),
            &chunks
        ));
    }
}
