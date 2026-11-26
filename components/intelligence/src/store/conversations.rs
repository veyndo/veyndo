//! Merchant conversations in storage: the caller's list, message history with approval state and owner/rights admission. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// A conversation in the caller's list.
pub struct ConversationItem {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

/// The caller's 40 latest conversations whose runs need no scope beyond `held`; owners also see unowned ones.
pub async fn list(
    e: impl PgExecutor<'_>,
    tenant: &str,
    owner: Option<&str>,
    shop_owner: bool,
    held: &Value,
) -> Result<Vec<ConversationItem>> {
    Ok(sqlx::query_as!(ConversationItem, r#"SELECT id,title,created_at::text AS "created_at!" FROM conversations WHERE tenant=$1 AND (owner_id=$2 OR (owner_id IS NULL AND $3)) AND NOT EXISTS(SELECT 1 FROM tasks k WHERE k.tenant=conversations.tenant AND k.conversation_id=conversations.id AND k.run_scopes IS NOT NULL AND NOT(k.run_scopes <@ $4)) ORDER BY created_at DESC LIMIT 40"#,
        tenant, owner, shop_owner, held).fetch_all(e).await?)
}

/// A conversation message with the approval state of its task.
pub struct Message {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub data: Value,
    pub applied: bool,
}

/// The last 200 messages of a conversation, oldest first.
pub async fn messages(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<Vec<Message>> {
    Ok(sqlx::query_as!(Message, r#"SELECT m.id AS "id!",m.role AS "role!",m.content AS "content!",m.data AS "data!",coalesce(k.applied,false) AS "applied!" FROM (SELECT * FROM chat_messages WHERE tenant=$1 AND conversation_id=$2 ORDER BY id DESC LIMIT 200) m LEFT JOIN tasks k ON k.tenant=m.tenant AND k.id=m.data->>'taskId' WHERE m.tenant=$1 AND m.conversation_id=$2 ORDER BY m.id LIMIT 200"#,
        tenant, id).fetch_all(e).await?)
}

/// Whether the caller may read the conversation with the scopes they hold; None when it is not theirs.
pub async fn admitted(
    e: impl PgExecutor<'_>,
    tenant: &str,
    id: &str,
    owner: Option<&str>,
    shop_owner: bool,
    held: &Value,
) -> Result<Option<bool>> {
    Ok(sqlx::query_scalar!(r#"SELECT NOT EXISTS(SELECT 1 FROM tasks k WHERE k.tenant=c.tenant AND k.conversation_id=c.id AND k.run_scopes IS NOT NULL AND NOT(k.run_scopes <@ $5)) AS "allowed!" FROM conversations c WHERE tenant=$1 AND id=$2 AND (owner_id=$3 OR (owner_id IS NULL AND $4))"#,
        tenant, id, owner, shop_owner, held).fetch_optional(e).await?)
}
