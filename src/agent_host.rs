//! The server side of the agent port: native capabilities, installed app tools, consent-bound preferences, studio order facts, the native work vocabulary and proposal admission.
use crate::{
    App, agent_runs, apps, auth, capabilities, cognition, commerce, operations, staging, studio,
};
use serde_json::Value;
use serde_json::json;
use veyndo_core::PortFuture;
use veyndo_core::hash;
use veyndo_core::request_context::RequestContext;
use veyndo_intelligence::ports::{AgentHost, WorkSchemas};
use veyndo_intelligence::service::models::Models;
use veyndo_intelligence::store::preferences::AdviceContext;
use veyndo_pricing::catalog::Product;
use veyndo_studio::domain::proposal::Proposal;

/// The server lends its state, tools and native review to agent work.
pub(crate) struct Host<'a>(pub(crate) &'a App);
impl AgentHost for Host<'_> {
    type Plan = Proposal;
    fn models(&self) -> Models<'_> {
        self.0.models()
    }
    fn reads(&self, h: &RequestContext) -> Vec<Value> {
        capabilities::agent_reads(self.0, h)
    }
    fn tool_schema(&self, name: &str) -> Value {
        capabilities::tool_schema(name)
    }
    fn read<'a>(
        &'a self,
        h: &'a RequestContext,
        name: &'a str,
        input: &'a Value,
    ) -> PortFuture<'a, Value> {
        capabilities::invoke(self.0, h, name, input)
    }
    fn app_tools<'a>(&'a self, h: &'a RequestContext, public: bool) -> PortFuture<'a, Vec<Value>> {
        Box::pin(apps::app_tools_for(self.0, h, public))
    }
    fn app_read<'a>(
        &'a self,
        h: &'a RequestContext,
        name: &'a str,
        input: &'a Value,
        resource: Option<&'a str>,
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::invoke_mcp_read(self.0, h, name, input, resource))
    }
    fn advice_context<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, AdviceContext> {
        Box::pin(cognition::preferences::capture(self.0, h))
    }
    fn planning_context<'a>(
        &'a self,
        t: &'a str,
        h: &'a RequestContext,
        instruction: &'a str,
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::planning_context(self.0, t, h, instruction))
    }
    fn planning_contracts<'a>(
        &'a self,
        t: &'a str,
        h: &'a RequestContext,
        ids: &'a [String],
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::planning_contracts(self.0, t, h, ids))
    }
    fn private_evidence<'a>(&'a self, t: &'a str, instruction: &'a str) -> PortFuture<'a, Value> {
        Box::pin(apps::private_evidence(self.0, t, instruction))
    }
    fn order_facts<'a>(&'a self, t: &'a str) -> PortFuture<'a, Value> {
        Box::pin(studio::facts::load(self.0, t))
    }
    fn work_schemas(&self, h: &RequestContext) -> WorkSchemas {
        WorkSchemas {
            navigation: agent_runs::navigation::schema(),
            view: agent_runs::view::schema(auth::allowed(h, "knowledge.read")),
            operations: agent_runs::work_plan::schema(),
        }
    }
    fn work_catalogue(&self, h: &RequestContext) -> Value {
        json!(agent_runs::work_plan::catalogue(self.0, h))
    }
    fn admit<'a>(
        &'a self,
        t: &'a str,
        h: &'a RequestContext,
        prepared: &'a Value,
        locale: &'a str,
        candidate: &'a Value,
    ) -> PortFuture<'a, (Proposal, Vec<Product>, i64)> {
        Box::pin(veyndo_studio::service::proposal_admission::admit(
            self, t, h, prepared, locale, candidate,
        ))
    }
    fn configuration_digest<'a>(&'a self, t: &'a str) -> PortFuture<'a, String> {
        Box::pin(async move {
            let mut tx = self.0.db.begin().await?;
            sqlx::query("SET LOCAL lock_timeout='250ms'")
                .execute(&mut *tx)
                .await?;
            operations::lock_company(&mut tx, t).await?;
            let configuration = staging::snapshot(&mut tx, t).await?;
            // Keep only the digest in the actor-private task, not a second configuration store.
            let digest = hash(&configuration.to_string());
            tx.rollback().await?;
            Ok(digest)
        })
    }
    fn fill_test_stock<'a>(&'a self, t: &'a str, quantity: i32) -> PortFuture<'a, Value> {
        Box::pin(commerce::test_stock::fill(self.0, t, quantity))
    }
}
impl veyndo_studio::ports::WorkHost for Host<'_> {
    fn work_operations(&self, h: &RequestContext) -> Vec<(&'static str, Value)> {
        capabilities::OPERATIONS
            .iter()
            .filter(|o| o.admitted(self.0, h))
            .map(|o| (o.name, o.input_schema()))
            .collect()
    }
    fn bind_change<'a>(
        &'a self,
        t: &'a str,
        change: &'a mut apps::AppChange,
        h: &'a RequestContext,
    ) -> PortFuture<'a, ()> {
        Box::pin(apps::bind_change(self.0, t, change, h))
    }
    fn apply_change<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        change: &'a apps::AppChange,
        h: &'a RequestContext,
    ) -> PortFuture<'a, ()> {
        Box::pin(apps::apply_change(tx, t, change, h))
    }
    fn catalog(&self) -> veyndo_catalog::ports::Catalog {
        axum::extract::FromRef::from_ref(self.0)
    }
}
