//! Shared commerce reports for Studio, the workspace, the API and agent reads; there is no second analytics ledger.
use crate::store::reports::{Context, run};
use serde_json::Value;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_identity::domain::permissions::allowed;
use veyndo_identity::merchant;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::config;

/// Whether a request may read shop reports: the overview or the order right.
pub fn visible(h: &RequestContext) -> bool {
    allowed(h, "overview.read") || allowed(h, "orders.read")
}

/// Runs one report in the reader's language with the behaviour consent filter.
pub async fn invoke(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    if !visible(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Shop reporting permission required".into(),
        ));
    }
    let (settings, _) = config(a, &t).await?;
    let (locale, chain) = language_context(a, h).await?;
    run(
        a,
        h,
        &t,
        v,
        Context {
            settings: &settings,
            locale: &locale,
            chain: &chain,
            eligible: &veyndo_intelligence::store::behavior_policies::eligible_per_channel(),
            conditions: &veyndo_marketing::service::conditions::validate_condition,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::visible;
    use veyndo_core::RequestContext;

    #[test]
    fn reports_need_the_overview_or_the_order_right() {
        let mut viewer = RequestContext::new();
        viewer.insert("x-rac-role", "viewer".parse().unwrap());
        assert!(visible(&viewer));
        let mut delegate = RequestContext::new();
        delegate.insert("x-rac-role", "viewer".parse().unwrap());
        delegate.insert("x-rac-permissions", r#"["catalog.read"]"#.parse().unwrap());
        assert!(!visible(&delegate));
        assert!(!visible(&RequestContext::new()));
    }
}
