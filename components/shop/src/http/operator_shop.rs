//! Operator shop dossier: registration, business identity, access roster, channels and measured HTTP activity.
use super::operator_metrics;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::channel_metrics::traffic;
use veyndo_core::host_names::links;
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
    let row = sqlx::query(
        "SELECT created_at::text AS registered,status,status_revision FROM tenants WHERE id=$1",
    )
    .bind(&id)
    .fetch_one(&a.db)
    .await?;
    data["createdAt"] = json!(row.get::<String, _>("registered"));
    data["status"] = json!(row.get::<String, _>("status"));
    data["statusRevision"] = json!(row.get::<i64, _>("status_revision"));
    data["urls"] = links(&id);
    let row=sqlx::query("SELECT (SELECT count(*) FROM products WHERE tenant=$1) AS products,(SELECT count(*) FROM customers WHERE tenant=$1) AS customers,(SELECT count(*) FROM app_packages WHERE tenant=$1 AND active) AS apps").bind(&id).fetch_one(&a.db).await?;
    data["counts"] = json!({"products":row.get::<i64,_>("products"),"customers":row.get::<i64,_>("customers"),"apps":row.get::<i64,_>("apps")});
    let members=sqlx::query("SELECT u.name,u.email,m.role,m.active FROM memberships m JOIN merchant_users u ON u.id=m.user_id WHERE m.tenant=$1 ORDER BY u.name LIMIT 100").bind(&id).fetch_all(&a.db).await?;
    data["members"]=json!(members.iter().map(|r|json!({"name":r.get::<String,_>("name"),"email":r.get::<String,_>("email"),"role":r.get::<String,_>("role"),"active":r.get::<bool,_>("active")})).collect::<Vec<_>>());
    let channels =
        sqlx::query("SELECT id,data FROM sales_channels WHERE tenant=$1 ORDER BY id LIMIT 100")
            .bind(&id)
            .fetch_all(&a.db)
            .await?;
    data["salesChannels"] = json!(
        channels
            .iter()
            .map(|r| json!({"id":r.get::<String,_>("id"),"configuration":r.get::<Value,_>("data")}))
            .collect::<Vec<_>>()
    );
    data["traffic"] = json!(traffic(&a.db, Some(&id), false).await?);
    // Receipt settings use the validated company schema, distinct from connector configuration.
    let settings: Option<Value> =
        sqlx::query_scalar("SELECT data FROM receipt_settings WHERE tenant=$1")
            .bind(&id)
            .fetch_optional(&a.db)
            .await?;
    data["business"] = settings.unwrap_or(json!({}));
    Ok(Json(data))
}
