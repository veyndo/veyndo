//! App cart contributions are owned by the apps component; the server projects the configured cart.
use crate::cart_price::cart_json;
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) use veyndo_shop::types::cart::AppConfiguration as Configuration;
pub(crate) async fn configure(a: &App, h: &RequestContext, id: &str, v: &Value) -> Result<Value> {
    let cart = veyndo_app_platform::service::cart_contributions::configure(a, h, id, v).await?;
    cart_json(a, &cart).await
}
pub(crate) use veyndo_app_platform::store::cart_configurations::{
    prepare_configurations, validate_configurations,
};
