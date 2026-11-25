//! Operator shop dossier: registration, business identity, access roster, channels and measured HTTP activity.
use super::operator_metrics;
use crate::store::operator_control;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde_json::{Value, json};
use veyndo_core::host_names::links;
use veyndo_core::store::channel_metrics::traffic;
use veyndo_core::{RequestContext, Result};

/// The operator dossier of one shop: registration, counts, access roster, channels, traffic and business identity.
pub async fn detail(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Query(c): Query<operator_metrics::Criteria>,
) -> Result<Json<Value>> {
    let Json(mut data) =
        operator_metrics::detail(State(a.clone()), h, Path(id.clone()), Query(c)).await?;
    let registration = operator_control::registration(&a, &id).await?;
    let counts = operator_control::counts(&a, &id).await?;
    let members = operator_control::members(&a, &id).await?;
    let channels = operator_control::channels(&a, &id).await?;
    let traffic = traffic(&a.db, Some(&id), false).await?;
    // Receipt settings use the validated company schema, distinct from connector configuration.
    let settings = operator_control::receipt_settings(&a, &id).await?;
    if let Some(dossier) = data.as_object_mut() {
        dossier.extend([
            ("createdAt".to_owned(), json!(registration.registered)),
            ("status".to_owned(), json!(registration.status)),
            (
                "statusRevision".to_owned(),
                json!(registration.status_revision),
            ),
            ("urls".to_owned(), links(&id)),
            (
                "counts".to_owned(),
                json!({"products":counts.products,"customers":counts.customers,"apps":counts.apps}),
            ),
            (
                "members".to_owned(),
                json!(members.iter().map(|r|json!({"name":r.name,"email":r.email,"role":r.role,"active":r.active})).collect::<Vec<_>>()),
            ),
            (
                "salesChannels".to_owned(),
                json!(
                    channels
                        .iter()
                        .map(|r| json!({"id":r.id,"configuration":r.data}))
                        .collect::<Vec<_>>()
                ),
            ),
            ("traffic".to_owned(), json!(traffic)),
            ("business".to_owned(), settings.unwrap_or(json!({}))),
        ]);
    }
    Ok(Json(data))
}
