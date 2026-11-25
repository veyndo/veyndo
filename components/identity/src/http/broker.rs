//! Password setup, recovery discovery and session requests from the trusted, verified-email identity broker.
//! No public reset-by-email endpoint: assertions bind route, identity, shop, action, expiry and one-use nonce.
use crate::domain::credentials::{email, hash_password, password, verify_password};
use crate::store::broker::assertion;
use crate::store::sessions::{issue_session, issue_session_with_proof};
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, Result, StatusCode, bad, conflict, validate_tenant};

/// Sets or recovers a password, or verifies the existing one, for a broker-verified identity.
pub async fn credentials(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/credentials", &v).await?;
    let action = c["action"]
        .as_str()
        .ok_or(bad("Credential action required"))?;
    if !["set-password", "verify-password"].contains(&action) {
        return Err(bad("Unsupported credential action"));
    }
    let secret = password(&c)?;
    let shop = c["workspaceId"].as_str().ok_or(bad("Shop ID required"))?;
    validate_tenant(shop)?;
    let mut tx = a.db.begin().await?;
    // Personal recovery changes credentials, never the linked account's existing role.
    let row = sqlx::query("SELECT u.id,u.password_hash,u.password_initialized,u.credential_revision FROM merchant_users u JOIN merchant_identities i ON i.user_id=u.id JOIN memberships m ON m.user_id=u.id WHERE i.issuer=$1 AND i.subject=$2 AND u.email=$3 AND m.tenant=$4 AND m.active FOR UPDATE OF u")
        .bind(c["iss"].as_str().unwrap()).bind(c["sub"].as_str().unwrap())
        .bind(email(&c)?).bind(shop).fetch_optional(&mut *tx).await?
        .ok_or(Error(StatusCode::UNAUTHORIZED, "Merchant identity unavailable".into()))?;
    let user: String = row.get("id");
    if action == "verify-password" {
        if !verify_password(secret, row.get("password_hash")).await? {
            return Err(Error(
                StatusCode::UNAUTHORIZED,
                "Invalid credentials".into(),
            ));
        }
    } else {
        if row.get::<bool, _>("password_initialized")
            && c["credentialRevision"].as_i64() != Some(row.get("credential_revision"))
        {
            return Err(conflict(
                "Account credentials changed; request a fresh recovery link",
            ));
        }
        let saved = hash_password(secret).await?;
        sqlx::query(
            "UPDATE merchant_users SET password_hash=$1,password_initialized=true,credential_revision=credential_revision+1 WHERE id=$2",
        )
        .bind(saved)
        .bind(&user)
        .execute(&mut *tx)
        .await?;
        // Password recovery also invalidates older Studio sessions and unredeemed handoffs.
        sqlx::query("DELETE FROM user_sessions WHERE user_id=$1")
            .bind(&user)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM merchant_handoffs WHERE user_id=$1")
            .bind(&user)
            .execute(&mut *tx)
            .await?;
        veyndo_core::store::outbox::publish_on(&mut *tx, shop, "account.security.updated", &json!({"userId":user,"action":"password.changed","credentialRevision":row.get::<i64,_>("credential_revision")+1})).await?;
    }
    tx.commit().await?;
    Ok(Json(issue_session(&a, &user, shop).await?))
}
/// Lists the active workspaces of a broker-verified account; it never provisions a shop.
pub async fn lookup(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/recovery", &v).await?;
    let email = email(&c)?;
    let mut tx = a.db.begin().await?;
    let row =
        sqlx::query("SELECT id,credential_revision FROM merchant_users WHERE email=$1 FOR UPDATE")
            .bind(&email)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(row) = row else {
        return Ok(Json(json!({"workspaces":[]})));
    };
    let user: String = row.get("id");
    let revision: i64 = row.get("credential_revision");
    sqlx::query("INSERT INTO merchant_identities(issuer,subject,user_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(c["iss"].as_str().unwrap()).bind(c["sub"].as_str().unwrap()).bind(&user).execute(&mut *tx).await?;
    let linked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM merchant_identities WHERE issuer=$1 AND subject=$2 AND user_id=$3)").bind(c["iss"].as_str().unwrap()).bind(c["sub"].as_str().unwrap()).bind(&user).fetch_one(&mut *tx).await?;
    if !linked {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Merchant identity unavailable".into(),
        ));
    }
    sqlx::query("UPDATE merchant_users SET email_verified_at=now() WHERE id=$1")
        .bind(&user)
        .execute(&mut *tx)
        .await?;
    let rows=sqlx::query("SELECT t.id,t.name FROM membership_grants m JOIN tenants t ON t.id=m.tenant WHERE m.user_id=$1 AND m.active AND t.status='active' ORDER BY t.name LIMIT 100").bind(user).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"credentialRevision":revision,"workspaces":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name")})).collect::<Vec<_>>()}),
    ))
}
/// Hands a freshly verified external identity its existing personal session; MFA remains mandatory.
pub async fn session(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/session", &v).await?;
    let email = email(&c)?;
    let row=sqlx::query("SELECT u.id,m.tenant FROM merchant_users u JOIN membership_grants m ON m.user_id=u.id AND m.active JOIN tenants t ON t.id=m.tenant AND t.status='active' WHERE u.email=$1 ORDER BY m.tenant LIMIT 1").bind(email).fetch_optional(&a.db).await?;
    let Some(row) = row else {
        return Ok(Json(json!({"newAccount":true})));
    };
    sqlx::query(
        "UPDATE merchant_users SET email_verified_at=COALESCE(email_verified_at,now()) WHERE id=$1",
    )
    .bind(row.get::<String, _>("id"))
    .execute(&a.db)
    .await?;
    Ok(Json(
        issue_session_with_proof(
            &a,
            &row.get::<String, _>("id"),
            &row.get::<String, _>("tenant"),
            c["sessionToken"].as_str(),
        )
        .await?,
    ))
}
