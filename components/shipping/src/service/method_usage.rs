//! Tenant-scoped dependency preflight for delivery and payment methods.
use crate::store::method_usage::{Area, Usage, usage};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};

pub async fn dependencies(a: &State, h: &RequestContext, area: &str, id: &str) -> Result<Usage> {
    permit(h, "settings.read")?;
    let t = merchant(h)?;
    let area = Area::parse(area)?;
    let mut tx = a.db.begin().await?;
    let found = usage(&mut tx, &t, area, id, None).await?;
    tx.commit().await?;
    Ok(found)
}
