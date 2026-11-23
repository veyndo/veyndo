//! Durable bounded embedding jobs live in the intelligence component; these entries lend it the server's model context.
use super::*;
use veyndo_intelligence::service::indexing as service;
pub(crate) use veyndo_intelligence::service::indexing::enqueue;
pub(crate) use veyndo_intelligence::service::models::embedding_model as model;
pub(crate) async fn index_once(a: &App) -> Result<bool> {
    service::index_once(a.models()).await
}
