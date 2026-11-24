//! Durable outbox and audit projection worker.
use crate::{App, apps, cognition, marketing};
use serde_json::Value;
use veyndo_core::Result;
mod retention;

pub(crate) use veyndo_core::outbox::SUBSCRIBERS;

/// The subscribers that live outside the core crate.
struct Components;
impl veyndo_core::outbox::Projector for Components {
    fn project<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'static, sqlx::Postgres>,
        subscriber: &'a str,
        tenant: &'a str,
        event: i64,
        kind: &'a str,
        data: &'a Value,
    ) -> veyndo_core::outbox::Projection<'a> {
        Box::pin(async move {
            match subscriber {
                "cognition" => cognition::project(tx, tenant, event, kind, data).await,
                "apps" => apps::project_events(tx, tenant, event, kind).await,
                "flows" => marketing::project_flows(tx, tenant, event, kind, data).await,
                _ => unreachable!(),
            }
        })
    }
}

pub(crate) async fn consume_subscriber(a: &App, subscriber: &str) -> Result<bool> {
    let worked = veyndo_core::outbox::consume(a, subscriber, &Components).await?;
    retention::once(a).await?;
    Ok(worked)
}
