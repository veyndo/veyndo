//! The operator a control-plane request acts as; the server authenticates the personal session and sets this header.
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode};

/// The authenticated platform operator of this request.
pub fn actor(h: &RequestContext) -> Result<&str> {
    header(h, "x-rac-platform-user")
        .ok_or(Error(StatusCode::UNAUTHORIZED, "Operator required".into()))
}
