//! Read-only storefront shopping advisor.
use crate::agent_host::Host;
use crate::{
    App, Error, IntoResponse, Json, RequestContext, Result, State, StatusCode, Value, bad,
    conflict, json, performance, tenant,
};
use axum::{
    Extension,
    http::{HeaderName, header::CACHE_CONTROL},
    response::sse::{Event, KeepAlive, Sse},
};
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;

pub(crate) async fn concierge(
    State(a): State<App>,
    h: RequestContext,
    stream: Option<Extension<performance::AiStreamAdmission>>,
    Json(v): Json<Value>,
) -> Result<axum::response::Response> {
    let t = tenant(&h)?;
    let request = v["request"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 2000)
        .ok_or(bad("Request required, maximum 2000 characters"))?;
    let _slot = a.inference_slots.clone().try_acquire_owned().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Inference capacity busy".into(),
        )
    })?;
    let _cluster = crate::performance::cluster_lease::Lease::acquire(&a, &t, "model", 2).await?;
    crate::performance::reserve_ai_attempt(&a, &t).await?;
    let Some(Extension(admission)) = stream else {
        return Ok(Json(
            veyndo_intelligence::service::concierge::advise(&Host(&a), &h, &t, request, None)
                .await?,
        )
        .into_response());
    };
    let ai_budget = admission
        .take()
        .ok_or_else(|| conflict("Shopping advice already started"))?;
    let request = request.to_owned();
    let (updates, mut views) = mpsc::channel(9);
    let mut job = veyndo::tenant_scope::spawn(async move {
        let _budget = (_slot, _cluster, ai_budget);
        let host = Host(&a);
        // A disconnected browser drops the receiver. A stalled receiver cannot
        // hold capacity past the producer's deadline or the completed result.
        tokio::select! {
            _ = updates.closed() => None,
            result = tokio::time::timeout(
                Duration::from_secs(280),
                veyndo_intelligence::service::concierge::advise(&host, &h, &t, &request, Some(&updates)),
            ) => Some(result.unwrap_or_else(|_| {
                Err(Error(StatusCode::GATEWAY_TIMEOUT, "Shopping advice timed out".into()))
            })),
        }
    });
    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                biased;
                Some(views) = views.recv() => {
                    yield Ok::<_, Infallible>(event("app-views", views));
                }
                result = &mut job => {
                    while let Ok(views) = views.try_recv() {
                        yield Ok(event("app-views", views));
                    }
                    yield Ok(match result {
                        Ok(Some(Ok(value))) => event("complete", value),
                        Ok(Some(Err(error))) => event("error", json!({"errors":[error.problem()]})),
                        Ok(None) => break,
                        Err(_) => event("error", json!({"errors":[{"status":500,"detail":"Shopping advice interrupted"}]})),
                    });
                    break;
                }
            }
        }
    };
    Ok((
        [
            (CACHE_CONTROL, "private, no-store"),
            (HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response())
}

fn event(name: &str, value: Value) -> Event {
    Event::default().event(name).data(value.to_string())
}
