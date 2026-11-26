//! Single-use, expiring invitations. Acceptance verifies an existing account password.
use crate::domain::credentials::{
    email, hash_password, name, password, role_allowed, verify_password,
};
use crate::store::sessions::issue_session;
use crate::store::{invitations, members, merchant_accounts, roles};
use crate::{merchant, permit, validate_permissions};
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash, uid};

/// The JSON fields `invite_user` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InviteUserInput {
    #[serde(deserialize_with = "lenient")]
    role: Option<String>,
}

pub async fn invite_user(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: InviteUserInput = view(&v);
    let t = merchant(&h)?;
    permit(&h, "users")?;
    let role = input.role.as_deref().unwrap_or("viewer");
    let mut tx = a.db.begin().await?;
    veyndo_core::store::tenants::lock(&mut *tx, &t).await?;
    let h = roles::current(&mut tx, &h, &t).await?;
    let actor_role = if header(&h, "x-rac-role") == Some("owner") {
        "owner"
    } else {
        "admin"
    };
    if !role_allowed(actor_role, role) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cannot assign this role".into(),
        ));
    }
    let grants = roles::grants(&mut tx, &t, role).await?;
    validate_permissions(&h, &grants)?;
    let email = email(&v)?;
    let token = format!("{}{}", uid(), uid());
    let id = uid();
    invitations::create(
        &mut tx,
        (&id, &hash(&token)),
        (&t, &email, role),
        header(&h, "x-rac-user").unwrap(),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"id":id,"token":token,"role":role,"workspace":t,"expiresIn":86400,"emailSent":false}),
    ))
}
/// The JSON fields `accept_invite` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AcceptInviteInput {
    #[serde(deserialize_with = "lenient")]
    invitation_token: Option<String>,
}

pub async fn accept_invite(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: AcceptInviteInput = view(&v);
    let token = input
        .invitation_token
        .as_deref()
        .filter(|s| s.len() == 64)
        .ok_or(bad("Invitation token required"))?;
    let password = password(&v)?;
    let name = name(&v)?;
    let mut tx = a.db.begin().await?;
    let invite_tenant = invitations::pending_tenant(&mut tx, &hash(token))
        .await?
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "Invitation expired or redeemed".into(),
        ))?;
    veyndo_core::store::tenants::lock(&mut *tx, &invite_tenant).await?;
    let invite = invitations::lock_pending(&mut tx, &hash(token))
        .await?
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "Invitation expired or redeemed".into(),
        ))?;
    let email = invite.email.clone();
    let tenant = invite.tenant.clone();
    let creator = invite.created_by.clone();
    let grants = roles::grants(&mut tx, &tenant, &invite.role).await?;
    if creator != "bootstrap" {
        let member = members::active_grant(&mut tx, &tenant, &creator)
            .await?
            .ok_or(Error(
                StatusCode::FORBIDDEN,
                "Invitation creator lost access".into(),
            ))?;
        let mut actor = RequestContext::new();
        actor.insert("x-rac-role", member.role.parse().unwrap());
        let scopes: Value = member.permissions;
        if !scopes.is_null() {
            actor.insert("x-rac-permissions", scopes.to_string().parse().unwrap());
        }
        permit(&actor, "users")?;
        if !role_allowed(
            if header(&actor, "x-rac-role") == Some("owner") {
                "owner"
            } else {
                "admin"
            },
            &invite.role,
        ) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Invitation creator can no longer assign this role".into(),
            ));
        }
        validate_permissions(&actor, &grants)?;
    }
    let existing = merchant_accounts::by_email(&mut *tx, &email).await?;
    let user = if let Some(existing) = existing {
        if !verify_password(password, existing.password_hash).await? {
            return Err(Error(
                StatusCode::UNAUTHORIZED,
                "Existing account password required".into(),
            ));
        }
        existing.id
    } else {
        let user = uid();
        let saved = hash_password(password).await?;
        merchant_accounts::create(&mut tx, &user, (&email, &name), saved).await?;
        user
    };
    // Existing membership cannot be escalated or demoted by redeeming an older invitation.
    members::add(&mut tx, &user, &tenant, &invite.role).await?;
    invitations::redeem(&mut tx, &invite.id).await?;
    tx.commit().await?;
    Ok(Json(issue_session(&a, &user, &tenant).await?))
}
