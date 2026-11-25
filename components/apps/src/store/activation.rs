//! Native current-actor/trust/connected-frontend/commerce admission for the shared package-state transaction; original configuration lock order retained.
use crate::domain::{approval, consent};
use crate::ports::CommerceInstall;
use crate::store::packages::package_on;
use crate::store::{commerce_hooks, dependencies, native_lifecycle};
use veyndo_app_storage::{activation as shared, installation::HookFuture};
use veyndo_core::{Error, RequestContext, bad, conflict, process_config};

/// The admission of one activation or deactivation, run by the shared package-state change.
pub struct Host<'a> {
    pub tenant: &'a str,
    pub app: &'a str,
    pub actor: &'a RequestContext,
    pub commerce: &'a dyn CommerceInstall,
}
impl<'db> shared::StateHooks<'db> for Host<'_> {
    type Error = Error;
    fn prepare<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move { self.commerce.lock(tx, self.tenant).await })
    }
    fn activating<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            let m = package_on(&mut **tx, self.tenant, self.app, false).await?;
            consent::actor_permissions(self.actor, &m)?;
            let config = process_config::get();
            approval::installation(&m, &config.services)?;
            dependencies::check(tx, self.tenant, &m, &config.publishers).await?;
            commerce_hooks::admission(tx, self.tenant, &m).await?;
            native_lifecycle::admission(tx, self.tenant, &m).await
        })
    }
    fn deactivating<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            let used = sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND app_id=$2) AS "used!""#,
                self.tenant,
                self.app
            )
            .fetch_one(&mut **tx)
            .await?;
            if used {
                return Err(conflict(
                    "App is required by connected frontends; disconnect them first",
                ));
            }
            Ok(())
        })
    }
}
/// The response for a refused or failed state change.
pub fn error(e: shared::ChangeError<Error>) -> Error {
    match e {
        shared::ChangeError::Database(e) => e.into(),
        shared::ChangeError::Invalid(s) => bad(s),
        shared::ChangeError::Conflict(s) => conflict(s),
        shared::ChangeError::Host(e) => e,
    }
}
