//! Provider-specific read imports and notification mapping stay outside the commerce kernel.
use super::*;
pub use analytics::analytics;
pub use gmail::gmail;
pub use slack::slack;
use veyndo_provider_gmail::actions as gmail_actions;
use veyndo_provider_gmail::provider as gmail;
use veyndo_provider_google_analytics::actions as analytics_actions;
use veyndo_provider_google_analytics::provider as analytics;
use veyndo_provider_slack::actions as slack_actions;
use veyndo_provider_slack::provider as slack;

pub fn oauth(a: &str) -> Result<&'static OAuth> {
    match a {
        "google_analytics" => Ok(&analytics_actions::OAUTH),
        "gmail" => Ok(&gmail_actions::OAUTH),
        "slack" => Ok(&slack_actions::OAUTH),
        _ => Err(Error::Invalid("Unknown OAuth app")),
    }
}
pub fn settings(a: &str) -> Result<&'static Settings> {
    match a {
        "google_analytics" => Ok(&analytics_actions::SETTINGS),
        "gmail" => Ok(&gmail_actions::SETTINGS),
        "slack" => Ok(&slack_actions::SETTINGS),
        _ => Err(Error::Invalid("Unknown settings app")),
    }
}
pub async fn action(store: &Store, t: &str, a: &str, name: &str, v: &Value) -> Result<Value> {
    match a {
        "google_analytics" => analytics_actions::action(store, t, name, v).await,
        "gmail" => gmail_actions::action(store, t, name, v).await,
        "slack" => slack_actions::action(store, t, name, v).await,
        _ => Err(Error::Invalid("Unknown connector app")),
    }
}
pub async fn event(store: &Store, t: &str, a: &str, event: &Value) -> Result<Value> {
    match a {
        "email" => email::event(store, t, event).await,
        "slack" => slack_actions::event(store, t, event).await,
        _ => Ok(json!({"ignored":true})),
    }
}
