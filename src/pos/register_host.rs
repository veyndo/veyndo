//! The server side of the assisted-selling register port (POS and manual orders): the guarded cart, quotes, currencies, checkout context, purchase, catalogue and receipt owners of this process.
use crate::{App, RequestContext, Value};
use axum::extract::{FromRef, Json, Path, Query, State};
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::PortFuture;

pub(crate) struct Register<'s> {
    app: &'s App,
    carts: crate::cart_storage::Carts<'s>,
    quotes: crate::cart_price::Quotes<'s>,
}
impl<'s> Register<'s> {
    pub(crate) fn new(app: &'s App) -> Self {
        Self {
            app,
            carts: crate::cart_storage::Carts(app),
            quotes: crate::cart_price::Quotes(app),
        }
    }
}
impl veyndo_pos::ports::Register for Register<'_> {
    fn carts(&self) -> &dyn veyndo_shop::ports::Carts {
        &self.carts
    }
    fn quotes(&self) -> &dyn veyndo_checkout::ports::Quotes {
        &self.quotes
    }
    fn currency<'a>(
        &'a self,
        h: &'a RequestContext,
        name: &'a str,
        v: &'a Value,
    ) -> PortFuture<'a, Value> {
        Box::pin(crate::currencies::invoke(self.app, h, name, v))
    }
    fn select<'a>(&'a self, h: &'a RequestContext, v: Value) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(
                crate::commerce::select_checkout(State(self.app.clone()), h.clone(), Json(v))
                    .await?
                    .0,
            )
        })
    }
    fn checkout<'a>(&'a self, h: &'a RequestContext, key: &'a str) -> PortFuture<'a, Value> {
        Box::pin(crate::checkout(self.app, h, key))
    }
    fn catalog<'a>(
        &'a self,
        h: &'a RequestContext,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(
                crate::catalog_page(State(self.app.clone()), h.clone(), criteria)
                    .await?
                    .0,
            )
        })
    }
    fn detail<'a>(
        &'a self,
        h: &'a RequestContext,
        id: String,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(crate::commerce::product_detail(
                State(FromRef::from_ref(self.app)),
                h.clone(),
                Path(id),
                Query(criteria),
            )
            .await?
            .0)
        })
    }
    fn receipt<'a>(
        &'a self,
        h: &'a RequestContext,
        order: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Value> {
        Box::pin(crate::operations::receipt_snapshot(self.app, h, order, id))
    }
}
