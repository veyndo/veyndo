//! The server side of the knowledge port: model context, product detail, catalogue localization and private app evidence.
use crate::foundation::App;
use axum::extract::{FromRef, Path, Query, State};
use serde_json::Value;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::PortFuture;
use veyndo_core::request_context::RequestContext;
use veyndo_intelligence::service::models::Models;
use veyndo_pricing::catalog::Product;

pub(crate) struct Knowledge<'s>(pub(crate) &'s App);
impl veyndo_intelligence::ports::KnowledgeHost for Knowledge<'_> {
    fn models(&self) -> Models<'_> {
        self.0.models()
    }
    fn detail<'a>(&'a self, h: &'a RequestContext, id: &'a str) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(crate::commerce::product_detail(
                State(FromRef::from_ref(self.0)),
                h.clone(),
                Path(id.to_owned()),
                Query(CatalogCriteria::default()),
            )
            .await?
            .0)
        })
    }
    fn localize<'a>(
        &'a self,
        tenant: &'a str,
        chain: &'a [String],
        products: Vec<Product>,
    ) -> PortFuture<'a, Vec<Product>> {
        Box::pin(crate::localization::localize_products(
            self.0, tenant, chain, products,
        ))
    }
    fn evidence<'a>(&'a self, tenant: &'a str, query: &'a str) -> PortFuture<'a, Value> {
        Box::pin(crate::apps::private_evidence(self.0, tenant, query))
    }
    fn graph<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Value> {
        Box::pin(crate::apps::private_graph(self.0, tenant))
    }
}
