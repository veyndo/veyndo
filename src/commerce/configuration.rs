//! Tenant checkout configuration loading.
use super::Settings;
use crate::{App, performance};
use std::sync::Arc;
use veyndo_core::Result;

pub(crate) use veyndo_shop::domain::defaults::decode as decode_config;
pub(crate) async fn config(a: &App, t: &str) -> Result<(Arc<Settings>, i64)> {
    performance::settings(a, t, "default").await
}
