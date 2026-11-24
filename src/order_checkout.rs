//! Native checkout entry used by Store API, Studio, agents and POS.
use crate::{App, checkout as owner};
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) async fn checkout(a: &App, h: &RequestContext, key: &str) -> Result<Value> {
    owner::execute(a, h, key, None).await
}
