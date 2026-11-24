//! The translation worker step lives in the studio component; this entry lends the server's model context.
use super::*;
pub(crate) async fn once(a: &App) -> Result<bool> {
    veyndo_studio::service::translation_worker::once(a.models()).await
}
