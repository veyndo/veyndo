//! Password setup, recovery discovery and session requests from the trusted, verified-email identity broker.
//! No public reset-by-email endpoint: assertions bind route, identity, shop, action, expiry and one-use nonce.
use crate::domain::credentials::{email, hash_password, password, verify_password};
use crate::store::broker::{self, Claims, assertion};
use crate::store::merchant_accounts;
use crate::store::sessions::{self, issue_session, issue_session_with_proof};
use crate::types::events::PasswordChanged;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{Error, Result, StatusCode, bad, conflict, validate_tenant};

/// Sets or recovers a password, or verifies the existing one, for a broker-verified identity.
pub async fn credentials(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/credentials", &v).await?;
    let claims: Claims = view(&c);
    let action = claims
        .action
        .as_deref()
        .ok_or(bad("Credential action required"))?;
    if !["set-password", "verify-password"].contains(&action) {
        return Err(bad("Unsupported credential action"));
    }
    let secret = password(&c)?;
    let shop = claims
        .workspace_id
        .as_deref()
        .ok_or(bad("Shop ID required"))?;
    validate_tenant(shop)?;
    let mut tx = a.db.begin().await?;
    // Personal recovery changes credentials, never the linked account's existing role.
    let row = broker::recoverable(&mut tx, claims.subject(), &email(&c)?, shop)
        .await?
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Merchant identity unavailable".into(),
        ))?;
    let user = row.id.clone();
    if action == "verify-password" {
        if !verify_password(secret, row.password_hash).await? {
            return Err(Error(
                StatusCode::UNAUTHORIZED,
                "Invalid credentials".into(),
            ));
        }
    } else {
        if row.password_initialized && claims.credential_revision != Some(row.credential_revision) {
            return Err(conflict(
                "Account credentials changed; request a fresh recovery link",
            ));
        }
        let saved = hash_password(secret).await?;
        broker::set_password(&mut tx, &user, saved).await?;
        // Password recovery also invalidates older Studio sessions and unredeemed handoffs.
        sessions::revoke_sessions_and_handoffs(&mut tx, &user).await?;
        let changed = PasswordChanged {
            user_id: &user,
            action: "password.changed",
            credential_revision: row.credential_revision + 1,
        };
        veyndo_core::events::emit(&mut *tx, shop, &changed).await?;
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
    let row = broker::lock_by_email(&mut tx, &email).await?;
    let Some(row) = row else {
        return Ok(Json(json!({"workspaces":[]})));
    };
    let user = row.id;
    let revision = row.credential_revision;
    let claims: Claims = view(&c);
    let subject = claims.subject();
    merchant_accounts::link(&mut tx, subject, &user).await?;
    let linked = broker::linked(&mut tx, subject, &user).await?;
    if !linked {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Merchant identity unavailable".into(),
        ));
    }
    broker::mark_verified(&mut tx, &user).await?;
    let rows = broker::workspaces(&mut tx, &user).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"credentialRevision":revision,"workspaces":rows.iter().map(|r|json!({"id":r.id,"name":r.name})).collect::<Vec<_>>()}),
    ))
}
/// Hands a freshly verified external identity its existing personal session; MFA remains mandatory.
pub async fn session(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/session", &v).await?;
    let email = email(&c)?;
    let row = broker::first_workspace(&a, &email).await?;
    let Some(row) = row else {
        return Ok(Json(json!({"newAccount":true})));
    };
    broker::mark_verified_once(&a, &row.id).await?;
    Ok(Json(
        issue_session_with_proof(
            &a,
            &row.id,
            &row.tenant,
            view::<Claims>(&c).session_token.as_deref(),
        )
        .await?,
    ))
}
