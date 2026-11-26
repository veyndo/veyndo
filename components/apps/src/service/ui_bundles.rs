//! Operator-pinned, bounded self-contained UI bundles.
use crate::service::egress;
use veyndo_core::{Error, Result, StatusCode, bad, hash, process_config};

/// Fetches an HTML bundle of at most 1 MiB and requires the digest the operator approved.
pub async fn fetch(url: reqwest::Url, digest: &str) -> Result<String> {
    let mut response = egress::client(&url, &process_config::get().private_service_origins)
        .await?
        .get(url)
        .send()
        .await
        .map_err(|_| Error(StatusCode::BAD_GATEWAY, "App UI bundle unavailable".into()))?;
    if !response.status().is_success()
        || !response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/html"))
    {
        return Err(Error(
            StatusCode::BAD_GATEWAY,
            "App UI must return an HTML bundle".into(),
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| bad("Incomplete UI bundle"))?
    {
        if bytes.len() + chunk.len() > 1_048_576 {
            return Err(bad("App UI bundle exceeds 1 MiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let html = std::str::from_utf8(&bytes).map_err(|_| bad("App UI bundle must be UTF-8"))?;
    if hash(html) != digest.to_ascii_lowercase() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "App UI bundle changed; operator must review and approve its new digest".into(),
        ));
    }
    Ok(html.to_owned())
}
