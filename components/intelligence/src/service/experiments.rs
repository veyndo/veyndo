//! Experiment operations for HTTP and MCP: reading needs the knowledge right, changes need settings rights and explicit approval.
use crate::store::experiments as store;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel, header};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::config;

/// Whether an experiment operation changes state.
pub fn changes(name: &str) -> bool {
    ![
        "knowledge.experiments",
        "knowledge.experiment.suggest",
        "knowledge.experiment.report",
    ]
    .contains(&name)
}

/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
}

/// Runs one experiment operation.
pub async fn invoke(a: &State, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let input: InvokeInput = view(v);
    let t = merchant(h)?;
    permit(h, "knowledge.read")?;
    if name == "knowledge.experiments" {
        return store::list(a, &t).await;
    }
    if name == "knowledge.experiment.suggest" {
        let (settings, _) = config(a, &t).await?;
        return store::suggest(
            a,
            &t,
            store::Basis {
                pricing_currency: &settings.currencies.pricing_currency,
                enabled: &settings.currencies.enabled,
                main_locale: &settings.main_locale,
                channel: channel(h),
            },
        )
        .await;
    }
    if name == "knowledge.experiment.report" {
        return store::report(
            a,
            &t,
            input.id.as_deref().ok_or(bad("Experiment required"))?,
        )
        .await;
    }
    debug_assert!(changes(name));
    permit(h, "settings.write")?;
    if input.approve != Some(true) {
        return Err(bad("Explicit approve=true required"));
    }
    let actor = header(h, "x-rac-user").unwrap_or("integration");
    let currencies = if name == "knowledge.experiment.create" {
        config(a, &t).await?.0.currencies.enabled.clone()
    } else {
        vec![]
    };
    store::change(a, &t, name, v, actor, &currencies).await
}

#[cfg(test)]
mod tests {
    use super::changes;

    #[test]
    fn reads_are_told_apart_from_changes() {
        assert!(!changes("knowledge.experiments"));
        assert!(!changes("knowledge.experiment.report"));
        assert!(changes("knowledge.experiment.create"));
        assert!(changes("knowledge.experiment.stop"));
    }
}
