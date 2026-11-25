//! Aggregate-only control-plane reads: real tenants, bounded pages, explicit currencies and simulated/confirmed amounts.
use super::operator::actor;
use crate::store::operator_metrics;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::host_names::links;
use veyndo_core::store::channel_metrics::traffic;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, validate_tenant};

#[derive(Deserialize)]
/// The window, search and page of a control-plane read.
pub struct Criteria {
    #[serde(default)]
    after: String,
    #[serde(default)]
    search: String,
    limit: Option<i64>,
    days: Option<i64>,
}
fn window(c: &Criteria) -> Result<i64> {
    let days = c.days.unwrap_or(30);
    if !(1..=90).contains(&days) || c.search.len() > 100 || c.after.len() > 48 {
        return Err(bad("Invalid statistics window or search"));
    }
    Ok(days)
}
fn amounts(rows: &[operator_metrics::Amounts]) -> Vec<Value> {
    rows.iter().map(|r|json!({"currency":r.currency,"orders":r.orders,"booked":r.booked,"simulated":r.simulated,"captured":r.captured})).collect()
}
/// Aggregate figures across all shops for a bounded window.
pub async fn overview(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Query(c): Query<Criteria>,
) -> Result<Json<Value>> {
    actor(&h)?;
    let days = window(&c)?;
    let mut tx = a.db.begin_read_snapshot().await?;
    let row = operator_metrics::totals(&mut tx, days).await?;
    let money = operator_metrics::amounts(&mut *tx, days, None).await?;
    tx.commit().await?;
    let channels = traffic(&a.db, None, true).await?;
    Ok(Json(
        json!({"days":days,"generatedAt":row.generated,"shops":row.shops,"sandboxes":row.sandboxes,"merchantUsers":row.users,"products":row.products,"customers":row.customers,"orders":row.orders,"pendingEvents":row.pending_events,"amounts":amounts(&money),"channels":channels,"channelScope":"persisted lifetime HTTP requests, not visitors; counters include tests and merchant reads","paymentScope":"recorded gross orders and confirmed online captures; not net revenue or accounting settlement"}),
    ))
}
/// One page of shops with their counts.
pub async fn shops(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Query(c): Query<Criteria>,
) -> Result<Json<Value>> {
    actor(&h)?;
    let days = window(&c)?;
    let limit = c.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(bad("Page size must be 1..100"));
    }
    let rows = operator_metrics::shops(&a, &c.after, &c.search, limit + 1, days).await?;
    let more = rows.len() > limit as usize;
    let rows = &rows[..rows.len().min(limit as usize)];
    Ok(Json(
        json!({"elements":rows.iter().map(|r| json!({"id":r.id,"name":r.name,"createdAt":r.created,"status":r.status,"statusRevision":r.status_revision,"urls":links(&r.id),"products":r.products,"customers":r.customers,"members":r.members,"orders":r.orders,"apps":r.apps,"salesChannels":r.channels,"knowledgeSources":r.sources})).collect::<Vec<_>>(),"hasMore":more,"nextCursor":if more {rows.last().map(|r|r.id.clone())}else{None},"days":days}),
    ))
}
/// The amounts and the daily order timeline of one shop.
pub async fn detail(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Query(c): Query<Criteria>,
) -> Result<Json<Value>> {
    actor(&h)?;
    validate_tenant(&id)?;
    let days = window(&c)?;
    let name = operator_metrics::live_name(&a, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Shop not found".into()))?;
    let mut tx = a.db.begin_read_snapshot().await?;
    let money = operator_metrics::amounts(&mut tx, days, Some(&id)).await?;
    tx.commit().await?;
    let daily = operator_metrics::timeline(&id, days).await?;
    Ok(Json(
        json!({"id":id,"name":name,"days":days,"amounts":amounts(&money),"timeline":daily.iter().map(|r|json!({"day":r.day,"orders":r.orders})).collect::<Vec<_>>()}),
    ))
}
