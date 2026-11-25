//! One-time personal-session handoff to the Studio (no passwords or bearer tokens in links): ticket creation and one-use redemption.
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

/// The user a live handoff ticket names, read before the account lock is taken.
pub async fn ticket_user(tx: &mut sqlx::PgConnection, digest: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT user_id FROM merchant_handoffs WHERE digest=$1 AND expires_at>now()",
        digest
    )
    .fetch_optional(tx)
    .await?)
}

/// The person and shop of a consumed ticket.
pub struct Redeemed {
    pub user_id: String,
    pub tenant: String,
}

/// Consumes a live ticket whose personal session is still live.
pub async fn consume(tx: &mut sqlx::PgConnection, digest: &str) -> Result<Option<Redeemed>> {
    Ok(sqlx::query_as!(
        Redeemed,
        "DELETE FROM merchant_handoffs h USING user_sessions s WHERE h.digest=$1 AND h.expires_at>now() AND h.session_digest=s.digest AND s.expires_at>now() RETURNING h.user_id,h.tenant",
        digest
    )
    .fetch_optional(tx)
    .await?)
}

/// Whether the user is an active member of the shop with an initialised password.
pub async fn member_ready(tx: &mut sqlx::PgConnection, user: &str, tenant: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM membership_grants m JOIN merchant_users u ON u.id=m.user_id WHERE m.user_id=$1 AND m.tenant=$2 AND m.active AND u.password_initialized) AS "ready!""#,
        user,
        tenant
    )
    .fetch_one(tx)
    .await?)
}
