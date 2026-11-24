//! Source-record imports live in the studio component; the server registers their routes and lends the import port.
use crate::{App, Json, RequestContext, Result, State, Value, apps, assets, marketing};
use veyndo_core::PortFuture;
use veyndo_studio::ports::ImportHost;
pub(crate) use veyndo_studio::service::imports::schema;

/// The server lends imports its app record, image and promotion owners.
pub(crate) struct Imports<'a>(pub(crate) &'a App);
impl ImportHost for Imports<'_> {
    fn core(&self) -> &veyndo_core::State {
        &self.0.core
    }
    fn catalog(&self) -> veyndo_catalog::ports::Catalog {
        axum::extract::FromRef::from_ref(self.0)
    }
    fn import_record<'a>(
        &'a self,
        h: &'a RequestContext,
        id: &'a str,
        destination: &'a Value,
        digest: &'a str,
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::import_record(self.0, h, id, destination, digest))
    }
    fn inspect_record<'a>(
        &'a self,
        h: &'a RequestContext,
        id: &'a str,
        destination: &'a Value,
        digest: &'a str,
    ) -> PortFuture<'a, (Value, Option<Value>)> {
        Box::pin(apps::inspect_import_record(
            self.0,
            h,
            id,
            destination,
            digest,
        ))
    }
    fn import_image<'a>(
        &'a self,
        h: &'a RequestContext,
        product: &'a str,
        id: &'a str,
        metadata: &'a Value,
        bytes: Vec<u8>,
    ) -> PortFuture<'a, Value> {
        Box::pin(assets::import_image(
            self.0, h, product, id, metadata, bytes,
        ))
    }
    fn import_promotion<'a>(
        &'a self,
        h: &'a RequestContext,
        id: &'a str,
        data: &'a Value,
    ) -> PortFuture<'a, Value> {
        Box::pin(marketing::import_promotion(self.0, h, id, data))
    }
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_studio::service::imports::invoke(&Imports(a), h, v).await
}
pub(crate) async fn inspect(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_studio::service::imports::inspect(&Imports(a), h, v).await
}
pub(crate) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    invoke(&a, &h, &v).await.map(Json)
}
pub(crate) async fn inspect_route(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    inspect(&a, &h, &v).await.map(Json)
}
