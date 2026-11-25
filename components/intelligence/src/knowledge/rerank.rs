//! Optional TEI cross-encoder reranking on at most 24 tenant-hydrated texts; malformed or unavailable providers retain fused ordering.
use super::PgPool;
use crate::store::product_reads::texts;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};

/// The product a search hit names.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Hit {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

pub async fn apply(tenant: &str, query: &str, hits: &mut [Value], db: &PgPool) -> bool {
    let Ok(url) = std::env::var("RERANKER_URL") else {
        return false;
    };
    if hits.is_empty() {
        return false;
    }
    let ids = hits
        .iter()
        .filter_map(|v| view::<Hit>(v).id)
        .collect::<Vec<_>>();
    let Ok(rows) = texts(db, tenant, &ids).await else {
        return false;
    };
    let texts = hits
        .iter()
        .map(|hit| {
            rows.iter()
                .find(|r| Some(&r.id) == view::<Hit>(hit).id.as_ref())
                .map(|r| {
                    format!("{} {}", r.name, r.description)
                        .chars()
                        .take(4000)
                        .collect::<String>()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let http = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Reranker HTTP client")
    });
    let mut request = http
        .post(format!("{}/rerank", url.trim_end_matches('/')))
        .json(&json!({"query":query,"texts":texts,"raw_scores":false}));
    if let Ok(key) = std::env::var("RERANKER_API_KEY") {
        request = request.bearer_auth(key);
    }
    let Ok(response) = request.send().await else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let Ok(value) = veyndo_runtime::http_json::bounded(response, 65536).await else {
        return false;
    };
    let Ok(scores) = validate(&value, hits.len()) else {
        return false;
    };
    let mut scored = scores
        .into_iter()
        .zip(hits.iter().cloned())
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut reranked = Vec::with_capacity(scored.len());
    for (score, mut hit) in scored {
        // A hit that is not an object cannot carry its score; keep the original order then.
        if merge(&mut hit, json!({ "rerankScore": score })).is_err() {
            return false;
        }
        reranked.push(hit);
    }
    for (slot, hit) in hits.iter_mut().zip(reranked) {
        *slot = hit;
    }
    true
}
fn validate(value: &Value, count: usize) -> Result<Vec<f64>, ()> {
    let rows = value.as_array().filter(|v| v.len() == count).ok_or(())?;
    let mut scores = vec![None; count];
    for row in rows {
        let row = view::<Scored>(row);
        let i = row.index.ok_or(())? as usize;
        let score = row.score.filter(|v| v.is_finite()).ok_or(())?;
        if i >= count || scores[i].is_some() {
            return Err(());
        }
        scores[i] = Some(score);
    }
    scores.into_iter().map(|v| v.ok_or(())).collect()
}
/// One reranker score for the text at `index`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Scored {
    #[serde(deserialize_with = "lenient")]
    index: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    score: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::validate;

    use serde_json::json;

    #[test]
    fn reranking_rejects_duplicate_missing_or_outside_candidates() {
        assert_eq!(
            validate(&json!([{"index":1,"score":0.9},{"index":0,"score":0.1}]), 2).unwrap(),
            vec![0.1, 0.9]
        );
        assert!(validate(&json!([{"index":0,"score":1},{"index":0,"score":2}]), 2).is_err());
        assert!(validate(&json!([{"index":2,"score":1}]), 1).is_err());
        assert!(validate(&json!([]), 1).is_err());
    }
}
