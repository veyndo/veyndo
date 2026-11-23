//! The server side of saving marketing configuration: actor-private work templates, installed app actions and POS channel setup.
use super::*;
use veyndo_marketing::ports::ConfigurationHost;

/// Lends the marketing configuration service the server's owners of templates, apps and POS channels.
pub(crate) struct Configuration<'a>(pub(crate) &'a App);
impl ConfigurationHost for Configuration<'_> {
    fn core(&self) -> &veyndo_core::State {
        &self.0.core
    }
    fn validate_template<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        actor: &'a RequestContext,
        flow: &'a Flow,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            if let Some(template) = &flow.work_template {
                veyndo_studio::service::flow_templates::load(tx, tenant, actor, template).await?;
            }
            Ok(())
        })
    }
    fn validate_apps<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        actor: &'a RequestContext,
        flow: &'a Flow,
        lock: bool,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(veyndo_app_platform::service::flow_apps::validate_installed(
            tx, tenant, actor, flow, lock,
        ))
    }
    fn configure_channel<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        id: &'a str,
        data: &'a Value,
        created: bool,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(crate::pos::configure(tx, tenant, id, data, created))
    }
}
