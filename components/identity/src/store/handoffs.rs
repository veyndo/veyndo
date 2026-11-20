//! One-time personal-session handoff to the Studio; no passwords or bearer tokens in links.
use serde_json::{Value, json};
use veyndo_core::{Error, Result, State, StatusCode, hash, uid};

/// Issues a ticket that is valid for 60 seconds and bound to the live personal session behind `token`.
pub async fn create(a: &State, user: &str, tenant: &str, token: &str) -> Result<Value> {
    let initialized: bool =
        sqlx::query_scalar("SELECT password_initialized FROM merchant_users WHERE id=$1")
            .bind(user)
            .fetch_one(&a.db)
            .await?;
    if !initialized {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Merchant password setup required".into(),
        ));
    }
    let ticket = format!("{}{}", uid(), uid());
    sqlx::query("DELETE FROM merchant_handoffs WHERE expires_at<now()")
        .execute(&a.db)
        .await?;
    let n=sqlx::query("INSERT INTO merchant_handoffs(digest,user_id,tenant,expires_at,session_digest) SELECT $1,$2,$3,now()+interval '60 seconds',digest FROM user_sessions WHERE digest=$4 AND user_id=$2 AND expires_at>now()")
        .bind(hash(&ticket)).bind(user).bind(tenant).bind(hash(token)).execute(&a.db).await?.rows_affected();
    if n != 1 {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal session required".into(),
        ));
    }
    Ok(json!({"ticket":ticket,"expiresIn":60}))
}
