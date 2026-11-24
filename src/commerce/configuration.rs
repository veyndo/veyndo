//! Tenant checkout configuration loading.
use super::Settings;
use crate::foundation::App;
use crate::performance;
use std::sync::Arc;
use veyndo_core::Result;

pub(crate) async fn config(a: &App, t: &str) -> Result<(Arc<Settings>, i64)> {
    performance::settings(a, t, "default").await
}
