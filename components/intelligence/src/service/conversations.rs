//! Persistent grounded merchant conversations: each turn is a durable run, and history is read only with the rights its runs were admitted under.
use super::agent_controls::conversation_actor;
use super::agent_runs::{conversation_run, enqueue, wait};
use super::models::choice;
use serde_json::{Value, json};
use sqlx::Row;
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
    let rows=sqlx::query("SELECT id,title,created_at::text AS created_at FROM conversations WHERE tenant=$1 AND (owner_id=$2 OR (owner_id IS NULL AND $3)) AND NOT EXISTS(SELECT 1 FROM tasks k WHERE k.tenant=conversations.tenant AND k.conversation_id=conversations.id AND k.run_scopes IS NOT NULL AND NOT(k.run_scopes <@ $4)) ORDER BY created_at DESC LIMIT 40").bind(&t).bind(conversation_actor(h)).bind(header(h,"x-rac-role")==Some("owner")).bind(held_scopes(h)).fetch_all(&a.db).await?;
    Ok(
        json!({"conversations":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"title":r.get::<String,_>("title"),"createdAt":r.get::<String,_>("created_at")})).collect::<Vec<_>>()}),
    )
}
/// The last 200 messages of a conversation with the approval state of their tasks.
async fn messages(a: &State, t: &str, id: &str) -> Result<Vec<Value>> {
    // Task state is joined on read so approval remains visible after reload.
    let rows=sqlx::query("SELECT m.id,m.role,m.content,m.data,coalesce(k.applied,false) AS applied FROM (SELECT * FROM chat_messages WHERE tenant=$1 AND conversation_id=$2 ORDER BY id DESC LIMIT 200) m LEFT JOIN tasks k ON k.tenant=m.tenant AND k.id=m.data->>'taskId' WHERE m.tenant=$1 AND m.conversation_id=$2 ORDER BY m.id LIMIT 200").bind(t).bind(id).fetch_all(&a.db).await?;
    Ok(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"role":r.get::<String,_>("role"),"content":r.get::<String,_>("content"),"data":r.get::<Value,_>("data"),"applied":r.get::<bool,_>("applied")})).collect())
}
/// Reading and continuing share the same owner and original-rights admission before any history is read.
async fn admit(a: &State, t: &str, id: &str, h: &RequestContext) -> Result<()> {
    let allowed:Option<bool>=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM tasks k WHERE k.tenant=c.tenant AND k.conversation_id=c.id AND k.run_scopes IS NOT NULL AND NOT(k.run_scopes <@ $5)) FROM conversations c WHERE tenant=$1 AND id=$2 AND (owner_id=$3 OR (owner_id IS NULL AND $4))")
        .bind(t).bind(id).bind(conversation_actor(h)).bind(header(h,"x-rac-role")==Some("owner")).bind(held_scopes(h)).fetch_optional(&a.db).await?;
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
            format!(
                "{}: {}",
                v["role"].as_str().unwrap_or(""),
                v["content"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// Queues a merchant chat turn as a durable run; a background turn returns at once.
pub async fn chat(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    assistant_access(h)?;
    let t = merchant(h)?;
    permit(h, "knowledge.read")?;
    let instruction = v["message"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4000)
        .ok_or(bad("Message must contain 1..4000 characters"))?;
    let selection = choice(v)?;
    let id = if let Some(id) = v["conversationId"].as_str() {
        admit(a, &t, id, h).await?;
        id.to_string()
    } else {
        uid()
    };
    let history = history(&messages(a, &t, &id).await?);
    let queued=enqueue(a,h,json!({"instruction":instruction,"inference":selection,"history":history,"locale":language_context(a,h).await?.0,"channel":channel_id(h),"workspaceContext":v["workspaceContext"],"requestId":v["requestId"],"newConversation":v["conversationId"].is_null(),"requestedConversation":v["conversationId"]}),Some(&id)).await?;
    let id = queued["conversationId"].as_str().unwrap_or(&id).to_string();
    admit(a, &t, &id, h).await?;
    if v["background"] == true {
        return Ok(json!({"conversationId":id,"messages":messages(a,&t,&id).await?,"run":queued}));
    }
    complete(a, h, queued).await
}
/// Waits for a queued turn and returns the conversation with its run.
pub async fn complete(a: &State, h: &RequestContext, queued: Value) -> Result<Value> {
    let t = merchant(h)?;
    let id = queued["conversationId"]
        .as_str()
        .ok_or(bad("Saved conversation required"))?;
    let run = wait(
        a,
        h,
        queued["taskId"].as_str().ok_or(bad("Saved run required"))?,
    )
    .await?;
    admit(a, &t, id, h).await?;
    Ok(json!({"conversationId":id,"messages":messages(a,&t,id).await?,"run":run}))
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
