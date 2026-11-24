//! The commerce side of an app installation: the configuration lock, flows that survive a replacement and payment methods.
use super::Manifest;
use veyndo_app_platform::ports::{CommerceInstall, RuleFuture};
/// Installation reaches marketing and payments only through this host.
pub(crate) struct CommerceHost;
pub(crate) static COMMERCE: CommerceHost = CommerceHost;
impl CommerceInstall for CommerceHost {
    fn lock<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
    ) -> RuleFuture<'a> {
        Box::pin(crate::marketing::lock_config(tx, t))
    }
    fn install<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        m: &'a Manifest,
    ) -> RuleFuture<'a> {
        Box::pin(async move {
            crate::marketing::validate_app_replacement(tx, t, m).await?;
            crate::payments::install_methods(tx, t, m).await
        })
    }
}
