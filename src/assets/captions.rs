//! Caption route and the server's public egress; the intelligence component owns the proposal and its image evidence.
use crate::{App, Json, RequestContext, Result, State, Value, apps, capabilities};
pub(crate) use veyndo_intelligence::service::media_captions::schema;
/// The DNS-pinned public HTTPS client of this process.
pub(crate) struct Egress;
impl veyndo_intelligence::ports::PublicEgress for Egress {
    fn client<'a>(&'a self, url: &'a reqwest::Url) -> veyndo_core::PortFuture<'a, reqwest::Client> {
        Box::pin(apps::public_https_client(url))
    }
}
pub(super) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        capabilities::invoke(&a, &h, "merchant.media.caption", &v).await?,
    ))
}
pub(super) async fn prepare(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::media_captions::prepare(a.models(), &Egress, h, v).await
}
