//! The consent-bound preference graph lives in the intelligence component; the server registers its route.
use crate::{App, Router, get};
pub(crate) use veyndo_intelligence::http::preferences::capture;
use veyndo_intelligence::http::preferences::{delete, read, save};
pub(crate) fn router() -> Router<App> {
    Router::new().route(
        "/store-api/intelligence/preferences",
        get(read).put(save).delete(delete),
    )
}
