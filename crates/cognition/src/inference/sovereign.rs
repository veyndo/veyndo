//! Sovereign mode: one switch that admits only the self-operated model route and reports where each model call goes.
use super::{Inference, Provider, Value, env, json};
use std::net::IpAddr;

/// The fields of a reported model route that decide whether data leaves the deployment.
#[derive(serde::Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Route {
    #[serde(rename = "use")]
    purpose: Value,
    configured: Value,
    self_hosted: Value,
}

const REFUSED: &str = "Sovereign AI mode admits only self-hosted models";

/// The deployment variable locks the mode on; the operator setting cannot switch it off.
pub fn locked() -> bool {
    env::var("SOVEREIGN_AI").as_deref() == Ok("true")
}
fn declared(host: &str) -> bool {
    env::var("SOVEREIGN_MODEL_HOSTS").is_ok_and(|list| {
        list.split(',')
            .any(|entry| entry.trim().eq_ignore_ascii_case(host))
    })
}
/// A name is not a location: this accepts loopback, private networks, cluster-internal names and
/// hosts the deployment owner declared as their own. It does not resolve DNS.
pub fn self_hosted(endpoint: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if let Ok(address) = host.parse::<IpAddr>() {
        return match address {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
            IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
        } || declared(&host);
    }
    host == "localhost"
        || !host.contains('.')
        || [".localhost", ".internal", ".local", ".svc"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
        || declared(&host)
}
fn host(endpoint: &str) -> Option<String> {
    reqwest::Url::parse(endpoint)
        .ok()?
        .host_str()
        .map(str::to_string)
}
fn route(usage: &str, endpoint: Option<String>, sovereign: bool) -> Value {
    let own = endpoint.as_deref().map(self_hosted);
    json!({"use":usage,"configured":endpoint.is_some(),"host":endpoint.as_deref().and_then(host),
        "selfHosted":own,"admitted":endpoint.is_some() && (!sovereign || own == Some(true))})
}
impl Inference {
    pub(super) fn apply_sovereign(&mut self, stored: bool) {
        self.sovereign = locked() || stored;
        if self.sovereign {
            self.default_provider = Provider::Ollama;
            for id in ["openai", "anthropic"] {
                if !self.disabled.iter().any(|v| v == id) {
                    self.disabled.push(id.into());
                }
            }
        }
    }
    /// Checked immediately before any model request leaves the process.
    pub(super) fn admit(&self, provider: &str, endpoint: &str) -> Result<(), String> {
        if !veyndo_kernel::verified_kernel::sovereign_request_admissible(
            self.sovereign,
            provider == "ollama",
            self_hosted(endpoint),
        ) {
            return Err(REFUSED.into());
        }
        Ok(())
    }
    /// Embedding and reranker endpoints come from the environment; they pass the same gate.
    pub async fn admit_endpoint(&self, endpoint: &str) -> Result<(), String> {
        self.resolved().await?.admit("ollama", endpoint)
    }
    fn routes(&self) -> Vec<Value> {
        /// The admission flag of a route a sovereign platform withholds.
        #[derive(serde::Serialize)]
        struct Admission {
            admitted: bool,
        }
        let embedding = env::var("EMBEDDING_BASE_URL")
            .ok()
            .or(Some(self.ollama.clone()));
        let images = (env::var("IMAGE_GENERATION_ENABLED").as_deref() == Ok("true")
            && self.openai_key.is_some())
        .then(|| self.openai_url.clone());
        let mut image = route("images", images, self.sovereign);
        if self.sovereign {
            veyndo_core::document::merge(&mut image, Admission { admitted: false })
                .expect("a route is an object");
        }
        vec![
            route("text", Some(self.ollama.clone()), self.sovereign),
            route("embeddings", embedding, self.sovereign),
            route("reranker", env::var("RERANKER_URL").ok(), self.sovereign),
            image,
        ]
    }
    /// Merchant-visible report: host names only, never keys, paths or credentials.
    pub(super) fn sovereignty(&self) -> Value {
        let routes = self.routes();
        let leaking = routes
            .iter()
            .map(veyndo_core::document::view::<Route>)
            .any(|r| r.purpose != "images" && r.configured == true && r.self_hosted != true);
        json!({"enabled":self.sovereign,"lockedByDeployment":locked(),
            "refusedProviders":if self.sovereign { json!(["openai","anthropic"]) } else { json!([]) },
            "routes":routes,"holds":self.sovereign && !leaking,
            "meaning":"Veyndo refuses a model request unless it goes to the self-operated model route. Host names are judged by network range and declared hosts, not by DNS resolution; apps you install are separate services with their own connections."})
    }
    /// A switch that would fail closed on every call is refused at save time instead.
    pub fn sovereign_blockers(&self, text_endpoint: &str) -> Vec<&'static str> {
        let mut blockers = vec![];
        if !self_hosted(text_endpoint) {
            blockers.push("text");
        }
        if env::var("EMBEDDING_BASE_URL").is_ok_and(|url| !self_hosted(&url)) {
            blockers.push("embeddings");
        }
        if env::var("RERANKER_URL").is_ok_and(|url| !self_hosted(&url)) {
            blockers.push("reranker");
        }
        blockers
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_private_declared_or_internal_hosts_count_as_self_hosted() {
        for own in [
            "http://127.0.0.1:11434",
            "http://[::1]:11434",
            "http://localhost:11434",
            "http://10.2.3.4:8080",
            "https://192.168.1.9",
            "http://172.20.0.5",
            "http://[fd00::1]:80",
            "http://ollama:11434",
            "http://models.internal",
            "http://ollama.ai.svc",
        ] {
            assert!(self_hosted(own), "{own}");
        }
        for foreign in [
            "https://api.openai.com/v1",
            "https://api.anthropic.com/v1",
            "https://ollama.example.com",
            "http://8.8.8.8",
            "http://172.32.0.1",
            "https://internal.example.com",
            "https://localhost.example.com",
            "not a url",
        ] {
            assert!(!self_hosted(foreign), "{foreign}");
        }
    }
    #[test]
    fn sovereign_mode_refuses_hosted_providers_and_foreign_endpoints() {
        let mut inference = Inference::from_env(reqwest::Client::new());
        inference.default_provider = Provider::Openai;
        inference.openai_key = Some("synthetic".into());
        assert!(
            inference
                .admit("openai", "https://api.openai.com/v1")
                .is_ok()
        );
        inference.apply_sovereign(true);
        assert!(matches!(inference.default_provider, Provider::Ollama));
        assert!(inference.disabled.contains(&"openai".to_string()));
        assert!(inference.disabled.contains(&"anthropic".to_string()));
        assert!(inference.admit("ollama", "http://127.0.0.1:11434").is_ok());
        assert_eq!(
            inference.admit("ollama", "https://ollama.example.com"),
            Err(REFUSED.into())
        );
        assert!(inference.admit("openai", "http://127.0.0.1:1").is_err());
        inference.ollama = "http://127.0.0.1:11434".into();
        let report = inference.sovereignty();
        assert_eq!(report["enabled"], true);
        assert_eq!(report["routes"][0]["host"], "127.0.0.1");
        assert_eq!(report["routes"][3]["admitted"], false);
        assert!(!report.to_string().contains("synthetic"));
        let listed = inference.providers();
        assert_eq!(listed["providers"][2]["configured"], false);
        assert_eq!(listed["defaultProvider"], "ollama");
    }
}
