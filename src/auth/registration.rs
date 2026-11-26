//! Create an isolated merchant workspace from synthetic template data.
//! Registration cannot claim existing tenants or assign a global administrator.
use super::{email, hash_password, issue_session, name, password};
use crate::catalog_model::prototype_products;
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;
use std::env;
use std::sync::Arc;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::uid;
use veyndo_core::validate_tenant;
use veyndo_intelligence::knowledge;

pub(crate) async fn register_user(
    State(a): State<App>,
    Json(input): Json<super::dto::Register>,
) -> Result<Json<Value>> {
    let v = json!(input);
    if env::var("ALLOW_PUBLIC_SIGNUP").as_deref() == Ok("false") {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Public registration is closed; use a merchant invitation".into(),
        ));
    }
    let email = email(&v)?;
    let name = name(&v)?;
    let password = hash_password(password(&v)?).await?;
    let slug = input.workspace_id.as_str();
    validate_tenant(slug)?;
    let shop_name = input.workspace_name.as_deref().unwrap_or(slug).trim();
    if shop_name.is_empty() || shop_name.len() > 100 {
        return Err(bad("Workspace name must contain 1..100 characters"));
    }
    let demo_password = hash_password("demo-business".into()).await?;
    let user = uid();
    let mut tx = a.db.begin().await?;
    if !veyndo_identity::store::merchant_accounts::register(
        &mut tx,
        &user,
        (&email, &name),
        password,
    )
    .await?
    {
        return Err(conflict("Email already registered; sign in instead"));
    }
    let sandbox =
        super::provision::provision(&mut tx, &user, slug, shop_name, demo_password).await?;
    tx.commit().await?;
    a.apps.sandboxes.insert(slug.into(), Arc::new(sandbox));
    for p in prototype_products(&a, slug).await? {
        knowledge::sync_product_scoped(&a.db, slug, &json!(p)).await?;
    }
    knowledge::seed_relations(&a.db, slug).await?;
    Ok(Json(issue_session(&a, &user, slug).await?))
}
