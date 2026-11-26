//! Compatible SSE transport waits on the same persisted run; detached request handling retains native tenant scope.
use crate::agent;
use crate::auth::SecureRoutes;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::request_context::RequestContext;
pub(crate) fn router() -> Router<App> {
    Router::new().secure_route(
        "/api/agent/chat/stream",
        &[("POST", "assistant.use")],
        post(chat),
    )
}
/// The chat turn fields the stream checks before accepting it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ChatInput {
    #[serde(deserialize_with = "lenient")]
    message: Option<String>,
    background: Value,
}

/// Marks the accepted turn as running in the background; the stream waits on its run.
#[derive(Serialize)]
struct Background {
    background: bool,
}

/// The accepted turn the stream reports and waits on.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Accepted {
    conversation_id: Value,
    run: Value,
}

async fn chat(
    State(a): State<App>,
    h: RequestContext,
    Json(mut v): Json<Value>,
) -> Result<impl axum::response::IntoResponse> {
    merchant(&a, &h)?;
    agent::assistant_access(&h)?;
    let input: ChatInput = view(&v);
    if !input
        .message
        .as_deref()
        .is_some_and(|s| !s.is_empty() && s.len() <= 4000)
    {
        return Err(bad("Message must contain 1..4000 characters"));
    }
    // Commit acceptance (or return its HTTP error) before opening SSE. Waiting never enqueues another turn.
    let background = input.background == true;
    merge(&mut v, Background { background: true })?;
    let accepted = veyndo_intelligence::service::conversations::chat(&a, &h, &v).await?;
    let run: Accepted = view(&accepted);
    let progress = json!({"conversationId":run.conversation_id,"run":run.run});
    let mut job = veyndo::tenant_scope::spawn(async move {
        if background {
            Ok(accepted)
        } else {
            veyndo_intelligence::service::conversations::complete(&a, &h, run.run).await
        }
    });
    let stream = async_stream::stream! {
        yield Ok::<_,std::convert::Infallible>(Event::default().event("accepted").json_data(progress).expect("JSON acceptance"));
        let started=std::time::Instant::now();
        loop {
            tokio::select! {
                result=&mut job => {
                    let event=match result {
                        Ok(Ok(value))=>Event::default().event("complete").json_data(value),
                        Ok(Err(e))=>Event::default().event("error").json_data(json!({"errors":[e.problem()]})),
                        Err(_)=>Event::default().event("error").json_data(json!({"errors":[{"status":500,"detail":"Chat execution interrupted; reload its conversation before retrying"}]})),
                    };
                    yield Ok(event.expect("JSON event serialization"));
                    break;
                },
                _=tokio::time::sleep(std::time::Duration::from_secs(2))=>{
                    yield Ok(Event::default().event("waiting").json_data(json!({"elapsedSeconds":started.elapsed().as_secs()})).expect("JSON event serialization"));
                }
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[cfg(test)]
mod tests {
    use super::Background;
    use serde_json::json;
    use veyndo_core::document::merge;

    #[test]
    fn background_flag_keeps_the_indexed_shape() {
        let body = json!({"message":"Hello","conversationId":"c-1","background":false});
        let mut old = body.clone();
        old["background"] = json!(true);
        let mut new = body;
        merge(&mut new, Background { background: true }).unwrap();
        assert_eq!(new, old);
    }
}
