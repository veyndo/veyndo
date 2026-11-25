//! Persistent grounded merchant conversations: each turn is a durable run, and history is read only with the rights its runs were admitted under.
use super::agent_controls::conversation_actor;
use super::agent_runs::{conversation_run, enqueue, wait};
use super::models::choice;
use crate::store::conversations as store;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel as channel_id, header};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, uid};
use veyndo_identity::{SCOPES, allowed, assistant_access, merchant, permit};
use veyndo_shop::service::language::language_context;

/// The scopes the caller holds now, as saved with a run.
fn held_scopes(h: &RequestContext) -> Value {
    json!(SCOPES.iter().filter(|s| allowed(h, s)).collect::<Vec<_>>())
}
/// The caller's recent conversations whose runs they may still read.
pub async fn list(a: &State, h: &RequestContext) -> Result<Value> {
    assistant_access(h)?;
    let t = merchant(h)?;
    let rows = store::list(
        &a.db,
        &t,
        conversation_actor(h).as_deref(),
        header(h, "x-rac-role") == Some("owner"),
        &held_scopes(h),
    )
    .await?;
    Ok(
        json!({"conversations":rows.iter().map(|r|json!({"id":r.id,"title":r.title,"createdAt":r.created_at})).collect::<Vec<_>>()}),
    )
}
/// The last 200 messages of a conversation with the approval state of their tasks.
async fn messages(a: &State, t: &str, id: &str) -> Result<Vec<Value>> {
    // Task state is joined on read so approval remains visible after reload.
    let rows = store::messages(&a.db, t, id).await?;
    Ok(rows.iter().map(|r|json!({"id":r.id,"role":r.role,"content":r.content,"data":r.data,"applied":r.applied})).collect())
}
/// Reading and continuing share the same owner and original-rights admission before any history is read.
async fn admit(a: &State, t: &str, id: &str, h: &RequestContext) -> Result<()> {
    let allowed = store::admitted(
        &a.db,
        t,
        id,
        conversation_actor(h).as_deref(),
        header(h, "x-rac-role") == Some("owner"),
        &held_scopes(h),
    )
    .await?;
    match allowed {
        None => Err(Error(
            StatusCode::NOT_FOUND,
            "Conversation not found".into(),
        )),
        Some(false) => Err(Error(
            StatusCode::FORBIDDEN,
            "Conversation evidence requires its original permissions".into(),
        )),
        Some(true) => Ok(()),
    }
}
/// One conversation with its messages and the caller's latest run.
pub async fn conversation(a: &State, h: &RequestContext, id: &str) -> Result<Value> {
    assistant_access(h)?;
    let t = merchant(h)?;
    admit(a, &t, id, h).await?;
    let run = conversation_run(a, h, id).await?;
    Ok(json!({"conversationId":id,"messages":messages(a,&t,id).await?,"run":run}))
}
/// The last 16 messages as plain prompt history.
fn history(previous: &[Value]) -> String {
    previous[previous.len().saturating_sub(16)..]
        .iter()
        .map(|v| {
            let m = view::<HistoryMessage>(v);
            format!(
                "{}: {}",
                m.role.as_deref().unwrap_or(""),
                m.content.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// One merchant chat turn: the message, the conversation it continues and the run options.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ChatTurn {
    #[serde(deserialize_with = "lenient")]
    message: Option<String>,
    conversation_id: Value,
    workspace_context: Value,
    request_id: Value,
    #[serde(deserialize_with = "lenient")]
    background: Option<bool>,
}

/// Queues a merchant chat turn as a durable run; a background turn returns at once.
pub async fn chat(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    assistant_access(h)?;
    let t = merchant(h)?;
    permit(h, "knowledge.read")?;
    let input: ChatTurn = view(v);
    let instruction = input
        .message
        .as_deref()
        .filter(|s| !s.is_empty() && s.len() <= 4000)
        .ok_or(bad("Message must contain 1..4000 characters"))?;
    let selection = choice(v)?;
    let id = if let Some(id) = input.conversation_id.as_str() {
        admit(a, &t, id, h).await?;
        id.to_string()
    } else {
        uid()
    };
    let history = history(&messages(a, &t, &id).await?);
    let queued=enqueue(a,h,json!({"instruction":instruction,"inference":selection,"history":history,"locale":language_context(a,h).await?.0,"channel":channel_id(h),"workspaceContext":input.workspace_context,"requestId":input.request_id,"newConversation":input.conversation_id.is_null(),"requestedConversation":input.conversation_id}),Some(&id)).await?;
    let id = view::<Queued>(&queued).conversation_id.unwrap_or(id);
    admit(a, &t, &id, h).await?;
    if input.background == Some(true) {
        return Ok(json!({"conversationId":id,"messages":messages(a,&t,&id).await?,"run":queued}));
    }
    complete(a, h, queued).await
}
/// Waits for a queued turn and returns the conversation with its run.
pub async fn complete(a: &State, h: &RequestContext, queued: Value) -> Result<Value> {
    let t = merchant(h)?;
    let turn: Queued = view(&queued);
    let id = turn
        .conversation_id
        .as_deref()
        .ok_or(bad("Saved conversation required"))?;
    let run = wait(
        a,
        h,
        turn.task_id.as_deref().ok_or(bad("Saved run required"))?,
    )
    .await?;
    admit(a, &t, id, h).await?;
    Ok(json!({"conversationId":id,"messages":messages(a,&t,id).await?,"run":run}))
}

/// A stored message as prompt history reads it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct HistoryMessage {
    #[serde(deserialize_with = "lenient")]
    role: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content: Option<String>,
}

/// A queued conversation turn.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Queued {
    #[serde(deserialize_with = "lenient")]
    conversation_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    task_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_keeps_the_last_sixteen_messages() {
        let all = (0..20)
            .map(|i| json!({"role":"user","content":i.to_string()}))
            .collect::<Vec<_>>();
        let text = history(&all);
        assert_eq!(text.lines().count(), 16);
        assert!(text.starts_with("user: 4"));
        assert_eq!(history(&[]), "");
    }
}
