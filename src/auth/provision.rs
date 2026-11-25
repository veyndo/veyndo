//! Reusable synthetic shop provisioning for initial signup and additional shops owned by the same merchant.
use super::hash_password;
use crate::catalog_model::prototype_products;
use crate::foundation::App;
use crate::store::{platform, shops};
use crate::{accounts, auth, categories, demo_catalog};
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::env;
use std::sync::Arc;
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::hash;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_core::validate_tenant;
use veyndo_intelligence::knowledge;

pub(super) async fn provision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    slug: &str,
    shop_name: &str,
    demo_password: String,
) -> Result<Sandbox> {
    provision_shop(
        tx,
        user,
        slug,
        shop_name,
        demo_password,
        true,
        env::var("SEED_DEMO").as_deref() != Ok("false"),
    )
    .await
}
pub(crate) async fn provision_shop(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    slug: &str,
    shop_name: &str,
    demo_password: String,
    seed_catalog: bool,
    demo_customer: bool,
) -> Result<Sandbox> {
    if [
        "app",
        "www",
        "api",
        "admin",
        "mail",
        "platform",
        "experience",
    ]
    .contains(&slug)
    {
        return Err(bad("Reserved shop ID"));
    }
    shops::lock_id(tx, slug).await?;
    if shops::alias_taken(tx, slug).await? {
        return Err(conflict("Shop address is already used by a frontend"));
    }
    if !shops::create(tx, slug, shop_name).await? {
        return Err(conflict("Workspace ID already exists"));
    }
    shops::add_owner(tx, user, slug).await?;
    if seed_catalog {
        demo_catalog::insert(tx, slug).await?;
    }
    let settings: Value =
        serde_json::from_str(include_str!("../../fixtures/demo-settings.json")).unwrap();
    shops::insert_settings(tx, slug, settings).await?;
    shops::insert_experience(
        &mut **tx,
        slug,
        json!({"mode":"balanced","headline":"A considered wardrobe. Made for everyday."}),
        false,
    )
    .await?;
    for variant in ["discovery", "comparison"] {
        shops::insert_policy(&mut **tx, slug, variant, false).await?;
    }
    shops::seed_automation(&mut **tx, slug).await?;
    if demo_customer {
        shops::insert_demo_customer(&mut **tx, slug, &demo_password, false).await?;
        accounts::seed_demo_address(tx, slug).await?;
    }
    let wat = include_str!("../../extensions/company-limit.wat");
    let sandbox = Sandbox::new(wat).map_err(bad)?;
    shops::insert_extension(&mut **tx, slug, wat, &hash(wat), false).await?;
    categories::seed(tx, slug).await?;
    if seed_catalog {
        demo_catalog::categories(tx, slug).await?;
    }
    Ok(sandbox)
}
/// The JSON fields `create_workspace` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CreateWorkspaceInput {
    #[serde(deserialize_with = "lenient")]
    workspace_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    workspace_name: Option<String>,
}

pub(crate) async fn create_workspace(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CreateWorkspaceInput = view(&v);
    auth::permit(&h, "read")?;
    let user = header(&h, "x-rac-user")
        .filter(|u| *u != "bootstrap")
        .ok_or(bad("Use a personal merchant account"))?;
    // Only a live personal session may create another workspace or carry MFA proof.
    super::mfa::personal(&a, &h).await?;
    let slug = input
        .workspace_id
        .as_deref()
        .ok_or(bad("Workspace ID required"))?;
    validate_tenant(slug)?;
    let name = input
        .workspace_name
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 100)
        .ok_or(bad("Workspace name required"))?;
    let demo = hash_password("demo-business".into()).await?;
    let mut tx = a.db.begin().await?;
    let token = header(&h, "authorization").and_then(|v| v.strip_prefix("Bearer "));
    platform::lock_account(&mut tx, user).await?;
    if !super::mfa::trusted_session_tx(&mut tx, user, token).await? {
        return Err(veyndo_core::Error(
            axum::http::StatusCode::UNAUTHORIZED,
            "Personal session expired".into(),
        ));
    }
    let sandbox = provision(&mut tx, user, slug, name, demo).await?;
    tx.commit().await?;
    a.apps.sandboxes.insert(slug.into(), Arc::new(sandbox));
    for p in prototype_products(&a, slug).await? {
        knowledge::sync_product_scoped(&a.db, slug, &json!(p)).await?;
    }
    knowledge::seed_relations(&a.db, slug).await?;
    Ok(Json(
        super::sessions::issue_session_with_proof(&a, user, slug, token).await?,
    ))
}
