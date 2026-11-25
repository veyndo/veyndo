//! Operator-signed incoming events: tenant-bound HMAC, five-minute freshness and atomic replay receipts.
use crate::domain::input_schema;
use crate::domain::webhook_signature::verify;
use crate::store::secrets;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::store::environments::parent;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, process_config,
};

/// Accepts one signed, fresh webhook once and queues its declared event.
pub async fn receive(
    Shared(a): Shared<State>,
    Path((t, id, webhook)): Path<(String, String, String)>,
    h: RequestContext,
    body: Bytes,
) -> Result<Json<Value>> {
    let denied = || {
        Error(
            StatusCode::UNAUTHORIZED,
            "App webhook authentication required".into(),
        )
    };
    if body.len() > 16384 {
        return Err(bad("Webhook body exceeds 16 KiB"));
    }
    let header = |key| h.get(key).and_then(|v| v.to_str().ok()).ok_or_else(denied);
    let stamp = header("x-app-timestamp")?;
    let time = stamp.parse::<i64>().map_err(|_| denied())?;
    if time.abs_diff(chrono::Utc::now().timestamp()) > 300 {
        return Err(denied());
    }
    let key = header("x-app-event-id")?;
    if key.is_empty()
        || key.len() > 100
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(denied());
    }
    let text = std::str::from_utf8(&body).map_err(|_| bad("Webhook JSON must be UTF-8"))?;
    let digest = hash(text);
    let config = &process_config::get().webhook_keys;
    if parent(&a, &t).await?.is_some() {
        return Err(bad("Incoming webhooks are disabled in private sandboxes"));
    }
    let input: Value = serde_json::from_str(text).map_err(|_| bad("Invalid webhook JSON"))?;
    let mut tx = a.db.begin().await?;
    let row = sqlx::query(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
    )
    .bind(&t)
    .bind(&id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "App unavailable".into()))?;
    let m: Manifest =
        serde_json::from_value(row.get("manifest")).map_err(|_| bad("Invalid package"))?;
    let secret = secrets::resolve_tx(&mut tx, &t, &m, "webhook")
        .await?
        .or_else(|| config[&t][&id].as_str().map(str::to_owned))
        .ok_or_else(denied)?;
    verify(
        &secret,
        &format!("{t}\n{id}\n{webhook}\n{stamp}\n{key}\n{digest}"),
        header("x-app-signature")?,
    )?;

    let w = m
        .webhooks
        .iter()
        .find(|w| w.id == webhook)
        .ok_or(Error(StatusCode::NOT_FOUND, "Webhook not declared".into()))?;
    let action = m
        .actions
        .iter()
        .find(|a| a.name == w.action && a.handler == "emit")
        .ok_or(bad("Webhook action missing"))?;
    input_schema::validate(&action.input_schema, &input)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,19))")
        .bind(format!("{t}:{id}:{webhook}:{key}"))
        .execute(&mut *tx)
        .await?;
    if let Some(r)=sqlx::query("SELECT digest,event_id FROM app_webhook_receipts WHERE tenant=$1 AND app=$2 AND webhook=$3 AND request_key=$4").bind(&t).bind(&id).bind(&webhook).bind(key).fetch_optional(&mut *tx).await? {
        if r.get::<String,_>("digest")!=digest {return Err(conflict("Webhook event id already used for a different payload"));}
        return Ok(Json(json!({"accepted":true,"replayed":true,"eventId":r.get::<i64,_>("event_id")})));
    }
    let event = veyndo_core::store::outbox::publish_id_on(
        &mut *tx,
        &t,
        &format!("app.{id}.{}", w.action),
        &input,
    )
    .await?;
    sqlx::query("INSERT INTO app_webhook_receipts(tenant,app,webhook,request_key,digest,event_id) VALUES($1,$2,$3,$4,$5,$6)").bind(t).bind(id).bind(webhook).bind(key).bind(digest).bind(event).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"accepted":true,"replayed":false,"eventId":event}),
    ))
}
