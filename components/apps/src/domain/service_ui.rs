//! Where the UI of a service app is served from; only the operator configures it.
use serde_json::Value;

/// The UI address the operator configured for an app: HTTPS, or HTTP on the local machine only.
pub fn ui_url(id: &str, configured: &Value) -> Option<String> {
    let url = configured[id]["uiUrl"].as_str()?;
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() == "https"
        || parsed.scheme() == "http"
            && ["localhost", "127.0.0.1"].contains(&parsed.host_str().unwrap_or(""))
    {
        Some(url.into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ui_url;
    use serde_json::json;

    #[test]
    fn the_ui_address_is_https_or_local_http() {
        let services = json!({
            "secure":{"uiUrl":"https://apps.example/ui"},
            "local":{"uiUrl":"http://localhost:4100"},
            "plain":{"uiUrl":"http://apps.example/ui"},
            "broken":{"uiUrl":"not a url"},
            "none":{}
        });
        assert_eq!(
            ui_url("secure", &services).as_deref(),
            Some("https://apps.example/ui")
        );
        assert_eq!(
            ui_url("local", &services).as_deref(),
            Some("http://localhost:4100")
        );
        for id in ["plain", "broken", "none", "missing"] {
            assert_eq!(ui_url(id, &services), None, "{id}");
        }
    }
}
