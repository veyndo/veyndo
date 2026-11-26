//! What agent runs, planning and shopping advice need from the server: native read operations, installed app tools and contracts, private app evidence, order facts, the native work vocabulary, proposal admission and publication.
use crate::service::models::Models;
use crate::store::preferences::AdviceContext;
use serde_json::Value;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_pricing::catalog::Product;

/// The schemas the server contributes to a planning response.
pub struct WorkSchemas {
    /// Navigation to a Studio place.
    pub navigation: Value,
    /// A reviewable list view.
    pub view: Value,
    /// Native configuration work steps.
    pub operations: Value,
}

/// The server side of agent work; read rounds, the run journal and planning stay in this component.
pub trait AgentHost: Send + Sync {
    /// The admitted proposal the server's native review produces.
    type Plan: serde::Serialize + Send;
    /// The model context: state, providers and inference slots.
    fn models(&self) -> Models<'_>;
    /// The native read operations the actor may call.
    fn reads(&self, h: &RequestContext) -> Vec<Value>;
    /// The input schema of a native tool.
    fn tool_schema(&self, name: &str) -> Value;
    /// Invokes a native operation through the shared capability dispatcher.
    fn read<'a>(
        &'a self,
        h: &'a RequestContext,
        name: &'a str,
        input: &'a Value,
    ) -> PortFuture<'a, Value>;
    /// The installed app tools the actor may call, or only the shopper-visible ones.
    fn app_tools<'a>(&'a self, h: &'a RequestContext, public: bool) -> PortFuture<'a, Vec<Value>>;
    /// Calls an installed app's read tool.
    fn app_read<'a>(
        &'a self,
        h: &'a RequestContext,
        name: &'a str,
        input: &'a Value,
        resource: Option<&'a str>,
    ) -> PortFuture<'a, Value>;
    /// The consent-bound private preference context of the shopper's cart.
    fn advice_context<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, AdviceContext>;
    /// The installed apps' records and actions relevant to an instruction.
    fn planning_context<'a>(
        &'a self,
        t: &'a str,
        h: &'a RequestContext,
        instruction: &'a str,
    ) -> PortFuture<'a, Value>;
    /// The managed save contracts of the named apps.
    fn planning_contracts<'a>(
        &'a self,
        t: &'a str,
        h: &'a RequestContext,
        apps: &'a [String],
    ) -> PortFuture<'a, Value>;
    /// Merchant-private external sources relevant to an instruction.
    fn private_evidence<'a>(&'a self, t: &'a str, instruction: &'a str) -> PortFuture<'a, Value>;
    /// The shop's order facts.
    fn order_facts<'a>(&'a self, t: &'a str) -> PortFuture<'a, Value>;
    /// The schemas of navigation, list views and native work for the actor.
    fn work_schemas(&self, h: &RequestContext) -> WorkSchemas;
    /// The native configuration work operations the actor may propose.
    fn work_catalogue(&self, h: &RequestContext) -> Value;
    /// Native read-only admission of a model proposal: the plan, the products it touches and the policy revision.
    fn admit<'a>(
        &'a self,
        t: &'a str,
        h: &'a RequestContext,
        prepared: &'a Value,
        locale: &'a str,
        candidate: &'a Value,
    ) -> PortFuture<'a, (Self::Plan, Vec<Product>, i64)>;
    /// The digest of a sandbox's release configuration projection; it excludes credentials, stock and operational records.
    fn configuration_digest<'a>(&'a self, t: &'a str) -> PortFuture<'a, String>;
    /// Gives empty products of a private sandbox a test stock quantity.
    fn fill_test_stock<'a>(&'a self, t: &'a str, quantity: i32) -> PortFuture<'a, Value>;
}

/// Public network evidence: a DNS-pinned client that reaches only public HTTPS addresses, without proxies or redirects.
pub trait PublicEgress: Send + Sync {
    /// A pinned client for the origin of a public HTTPS URL.
    fn client<'a>(&'a self, url: &'a reqwest::Url) -> PortFuture<'a, reqwest::Client>;
}

/// What product questions, retrieval previews and the product knowledge view need from the server: the model context, the product detail, catalogue localization and private app evidence.
pub trait KnowledgeHost: Send + Sync {
    /// The model context: state, providers and inference slots.
    fn models(&self) -> Models<'_>;
    /// The storefront product detail of the request, with its authoritative snapshot under `product`.
    fn detail<'a>(&'a self, h: &'a RequestContext, id: &'a str) -> PortFuture<'a, Value>;
    /// The products translated along the reader's language chain.
    fn localize<'a>(
        &'a self,
        tenant: &'a str,
        chain: &'a [String],
        products: Vec<Product>,
    ) -> PortFuture<'a, Vec<Product>>;
    /// Merchant-private app evidence matching a query.
    fn evidence<'a>(&'a self, tenant: &'a str, query: &'a str) -> PortFuture<'a, Value>;
    /// The merchant-private app evidence graph of the shop.
    fn graph<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Value>;
}

/// What catalogue intake needs from the server: models, public egress, provider admission, agentic readiness and the native product creation.
pub trait IntakeHost: Send + Sync {
    /// The model context: state, providers and inference slots.
    fn models(&self) -> Models<'_>;
    /// The DNS-pinned public HTTPS egress.
    fn egress(&self) -> &dyn PublicEgress;
    /// Admits one call of a bounded external provider for the shop; dropping the guard releases it.
    fn admit<'a>(
        &'a self,
        tenant: &'a str,
        provider: &'a str,
    ) -> PortFuture<'a, Box<dyn Send + Sync>>;
    /// The agentic commerce readiness of the request's channel.
    fn readiness<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, Value>;
    /// Creates one product through the native product editor.
    fn create<'a>(&'a self, h: &'a RequestContext, product: Value) -> PortFuture<'a, Value>;
}
