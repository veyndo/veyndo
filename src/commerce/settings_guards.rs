//! The server side of the settings port: rule conditions, payment capture configuration, the marketing configuration lock and app lifecycle locales.
use super::Settings;
use crate::{App, apps, marketing, payments};
use serde_json::Value;
use std::sync::Arc;
use veyndo_core::PortFuture;
use veyndo_core::Result;
use veyndo_shop::ports::{SettingsGuards, SettingsHost};

/// The server's checks for settings writes.
pub(crate) struct Guards;
impl SettingsGuards for Guards {
    fn condition(&self, v: &Value) -> Result<()> {
        marketing::validate_condition(v)
    }
    fn lock<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(marketing::lock_config(tx, t))
    }
    fn capture<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        settings: &'a Settings,
    ) -> PortFuture<'a, ()> {
        Box::pin(payments::capture_configuration::validate(tx, t, settings))
    }
    fn locale<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        locale: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(apps::native_lifecycle::validate_active_locale(
            tx, t, locale,
        ))
    }
}
impl axum::extract::FromRef<App> for SettingsHost {
    fn from_ref(app: &App) -> Self {
        SettingsHost {
            core: app.core.clone(),
            guards: Arc::new(Guards),
        }
    }
}
pub(crate) use veyndo_shop::service::settings_writes::{
    get_scope, merchant_config, save_config, save_scope,
};
