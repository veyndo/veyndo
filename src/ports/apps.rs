//! The app platform's directory: customer existence (customers) and personal preview expiry (studio), owners the app crate may not depend on.
use sqlx::PgConnection;
use veyndo_app_platform::ports::Directory as AppDirectory;
use veyndo_core::PortFuture;

/// Customer and preview-registry access for the app platform.
pub(crate) struct Directory;

impl AppDirectory for Directory {
    fn customer_exists<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        t: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, bool> {
        Box::pin(veyndo_customers::store::app_references::exists(conn, t, id))
    }

    fn extend_preview<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        environment: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_studio::store::app_previews::extend(
            conn,
            environment,
        ))
    }
}
