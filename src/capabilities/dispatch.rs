//! Closed native domain routing selected once by the operation registry; inactive futures stay boxed.
use super::{CapabilityFuture, invoke_native};
use crate::foundation::App;
use crate::{
    agent_runs, apps, assets, auth, b2b, cognition, commerce, currencies, developer, documents,
    history, legal, marketing, operations, payments, pos, studio,
};
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
/// The record a directly routed operation names.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DirectTarget {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
}
#[derive(Clone, Copy)]
pub(super) enum Owner {
    PaymentMethods,
    PaymentInstructions,
    CatalogIntake,
    Reports,
    Runs,
    Personal,
    Site,
    AppHistory,
    AppDiscovery,
    Roles,
    B2b,
    Pos,
    Facts,
    Cognitive,
    Currencies,
    Legal,
    History,
    Media,
    International,
    Automation,
    Operations,
    Developer,
    Knowledge,
    Planner,
    Native,
}
impl Owner {
    pub(super) fn for_name(name: &str) -> Self {
        if super::planner::schema(name).is_some() {
            Self::Planner
        } else if ["checkout.payment.methods", "checkout.payment.method.revoke"].contains(&name) {
            Self::PaymentMethods
        } else if name == "checkout.payment.instructions" {
            Self::PaymentInstructions
        } else if name == "merchant.reports.read" {
            Self::Reports
        } else if name.starts_with("merchant.apps.") {
            Self::AppDiscovery
        } else if name.starts_with("merchant.catalog.") {
            Self::CatalogIntake
        } else if name.starts_with("merchant.run.")
            || name.starts_with("merchant.agents.")
            || name.starts_with("merchant.scenario.")
        {
            Self::Runs
        } else if name.starts_with("studio.workspace.") {
            Self::Personal
        } else if name.starts_with("merchant.site.") {
            Self::Site
        } else if name.starts_with("merchant.app.") {
            Self::AppHistory
        } else if name.starts_with("team.role") {
            Self::Roles
        } else if name.starts_with("b2b.") {
            Self::B2b
        } else if name.starts_with("pos.") {
            Self::Pos
        } else if name == "catalog.facts" {
            Self::Facts
        } else if cognition::contracts::schema(name).is_some() {
            Self::Cognitive
        } else if currencies::schema(name).is_some() {
            Self::Currencies
        } else if legal::schema(name).is_some() {
            Self::Legal
        } else if history::schema(name).is_some() {
            Self::History
        } else if name.starts_with("merchant.media.") {
            Self::Media
        } else if commerce::international_permission(name).is_some() {
            Self::International
        } else if name.starts_with("automation.") {
            Self::Automation
        } else if operations::permission(name).is_some() {
            Self::Operations
        } else if name.starts_with("developer.") {
            Self::Developer
        } else if documents::knowledge_schema(name).is_some() {
            Self::Knowledge
        } else {
            Self::Native
        }
    }
    pub(super) fn invoke<'a>(
        self,
        a: &'a App,
        h: &'a RequestContext,
        name: &'a str,
        v: &'a Value,
    ) -> CapabilityFuture<'a> {
        match self {
            Self::PaymentMethods => Box::pin(payments::instruments::invoke(a, h, name, v)),
            Self::PaymentInstructions => Box::pin(async move {
                let target: DirectTarget = view(v);
                payments::customer_instructions(a, h, target.id.as_deref().unwrap_or("")).await
            }),
            Self::Reports => Box::pin(studio::reports::invoke(a, h, v)),
            Self::AppDiscovery => Box::pin(apps::discovery::invoke(a, h, name, v)),
            Self::CatalogIntake => Box::pin(commerce::catalog_intake::invoke(a, h, name, v)),
            Self::Runs => Box::pin(agent_runs::invoke(a, h, name, v)),
            Self::Personal => Box::pin(studio::personal::invoke(a, h, name, v)),
            Self::Site => Box::pin(async move {
                apps::site_builder::design_agent::invoke(
                    &apps::site_host::Site(a.clone()),
                    h,
                    name,
                    v,
                )
                .await
            }),
            Self::AppHistory => Box::pin(apps::record_history::invoke(a, h, name, v)),
            Self::Roles => Box::pin(auth::roles::invoke(a, h, name, v)),
            Self::B2b => Box::pin(b2b::invoke(a, h, name, v)),
            Self::Pos => Box::pin(pos::invoke(a, h, name.strip_prefix("pos.").unwrap(), v)),
            Self::Facts => Box::pin(async move {
                cognition::signed::facts(
                    a,
                    h,
                    view::<DirectTarget>(v)
                        .product_id
                        .as_deref()
                        .ok_or(bad("Product required"))?,
                )
                .await
            }),
            Self::Cognitive => Box::pin(cognition::contracts::invoke_contract(a, h, name, v)),
            Self::Currencies => Box::pin(currencies::invoke(a, h, name, v)),
            Self::Legal => Box::pin(legal::invoke(a, h, name, v)),
            Self::History => Box::pin(history::invoke(a, h, name, v)),
            Self::Media => Box::pin(assets::media_invoke(a, h, name, v)),
            Self::International => Box::pin(commerce::international_invoke(a, h, name, v)),
            Self::Automation => Box::pin(marketing::invoke(a, h, name, v)),
            Self::Operations => Box::pin(operations::invoke(a, h, name, v)),
            Self::Developer => Box::pin(developer::invoke(a, h, name, v)),
            Self::Knowledge => Box::pin(documents::knowledge_invoke(a, h, name, v)),
            Self::Planner => Box::pin(super::planner::invoke(h, name, v)),
            Self::Native => Box::pin(invoke_native(a, h, name, v)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn targets_read_like_indexing() {
        for v in [
            json!({}),
            json!({"id":"a","productId":"p"}),
            json!({"id":1,"productId":null}),
            json!("x"),
        ] {
            let target: DirectTarget = view(&v);
            assert_eq!(target.id.as_deref(), v["id"].as_str());
            assert_eq!(target.product_id.as_deref(), v["productId"].as_str());
        }
    }
}
