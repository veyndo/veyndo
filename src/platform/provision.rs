//! Operator shop creation commits ownership, settings and audit atomically; never issues another user's credentials.
use super::auth;
use crate::catalog_model::prototype_products;
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::validate_tenant;
use veyndo_intelligence::knowledge;
/// A new shop: its ID and name, an optional demo catalogue and an existing owner account.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NewShop {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    seed_catalog: Option<bool>,
    owner_email: Value,
}

pub(super) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let actor = auth::actor(&h)?;
    let input: NewShop = view(&v);
    let id = input.id.as_deref().ok_or(bad("Shop ID required"))?;
    validate_tenant(id)?;
    if ["app", "www", "api", "admin", "mail", "platform"].contains(&id) {
        return Err(bad("Reserved shop ID"));
    }
    let name = input
        .name
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 100)
        .ok_or(bad("Shop name required"))?;
    let seed = input.seed_catalog.unwrap_or(false);
    let owner = if input
        .owner_email
        .as_str()
        .is_some_and(|s| !s.trim().is_empty())
    {
        crate::store::platform::account_id_by_email(
            &a.db,
            &crate::auth::email(&json!({"email":input.owner_email}))?,
        )
        .await?
        .ok_or(bad("Owner must have an existing personal merchant account"))?
    } else {
        actor.to_string()
    };
    let mut tx = a.db.begin().await?;
    let sandbox =
        crate::auth::provision_shop(&mut tx, &owner, id, name.trim(), String::new(), seed, false)
            .await?;
    crate::store::platform::audit_shop_created(
        &mut tx,
        actor,
        id,
        json!({"owner":owner,"seedCatalog":seed}),
    )
    .await?;
    tx.commit().await?;
    a.apps.sandboxes.insert(id.into(), Arc::new(sandbox));
    // Product indexing uses the regular consumer and reports failure independently; committed shop ownership stays valid.
    let indexed = if seed {
        let result: Result<()> = async {
            for p in prototype_products(&a, id).await? {
                knowledge::sync_product_scoped(&a.db, id, &json!(p)).await?;
            }
            knowledge::seed_relations(&a.db, id).await?;
            Ok(())
        }
        .await;
        result.is_ok()
    } else {
        true
    };
    Ok(Json(
        json!({"id":id,"name":name.trim(),"ownerId":owner,"seedCatalog":seed,"knowledgeIndexed":indexed,"paymentMode":"simulated","urls":crate::shop_domains::links(id),"storefrontPath":format!("/?shop={id}"),"studioPath":format!("/?shop={id}#merchant")}),
    ))
}
