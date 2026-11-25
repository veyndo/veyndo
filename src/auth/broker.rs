//! Optional trusted identity exchange: signatures bind route, audience, expiry and one-use nonce.
use super::{email, hash_password, provision_shop};
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
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::uid;
use veyndo_core::validate_tenant;
use veyndo_intelligence::knowledge;

pub(crate) use veyndo_identity::store::broker::assertion;
use veyndo_identity::store::merchant_accounts as accounts;

/// The signed claims an identity exchange reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ExchangeClaims {
    #[serde(deserialize_with = "lenient")]
    iss: Option<String>,
    #[serde(deserialize_with = "lenient")]
    sub: Option<String>,
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    workspace_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    workspace_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    session_token: Option<String>,
    demo_catalog: Value,
}

pub(crate) async fn exchange(State(a): State<App>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/exchange", &v).await?;
    let claims: ExchangeClaims = view(&c);
    let issuer = claims.iss.as_deref().unwrap();
    let subject = claims.sub.as_deref().unwrap();
    let email = email(&c)?;
    let name = claims
        .name
        .as_deref()
        .filter(|n| !n.trim().is_empty() && n.len() <= 100)
        .ok_or(bad("Merchant name required"))?;
    let shop = claims
        .workspace_id
        .as_deref()
        .ok_or(bad("Shop ID required"))?;
    validate_tenant(shop)?;
    let shop_name = claims
        .workspace_name
        .as_deref()
        .filter(|n| !n.trim().is_empty() && n.len() <= 100)
        .ok_or(bad("Shop name required"))?;
    let random_password = hash_password(format!("{}{}", uid(), uid())).await?;
    let mut tx = a.db.begin().await?;
    let (user, linked) =
        accounts::broker_account(&mut tx, (issuer, subject), (&email, name), random_password)
            .await?;
    if linked.as_ref().is_some_and(|u| u != &user) {
        return Err(conflict(
            "Identity email changed; explicit account recovery required",
        ));
    }
    accounts::link(&mut tx, (issuer, subject), &user).await?;
    let first = accounts::first_shop(&mut *tx, &user).await?;
    if !super::mfa::trusted_session_tx(&mut tx, &user, claims.session_token.as_deref()).await?
        && let Some(first) = first
        && let Some(challenge) = super::mfa::challenge_tx(&mut tx, &user, &first).await?
    {
        tx.commit().await?;
        return Ok(Json(challenge));
    }
    let (exists, owned) = accounts::ownership(&mut tx, shop, &user).await?;
    let mut sandbox = None;
    let mut seeded = false;
    if exists {
        if !owned {
            return Err(conflict("Shop address already exists"));
        }
    } else {
        seeded = claims.demo_catalog == true;
        sandbox = Some(
            provision_shop(
                &mut tx,
                &user,
                shop,
                shop_name,
                String::new(),
                claims.demo_catalog == true,
                false,
            )
            .await?,
        );
    }
    tx.commit().await?;
    if let Some(sandbox) = sandbox {
        a.apps.sandboxes.insert(shop.into(), Arc::new(sandbox));
    }
    if seeded {
        for p in prototype_products(&a, shop).await? {
            knowledge::sync_product_scoped(&a.db, shop, &json!(p)).await?;
        }
        knowledge::seed_relations(&a.db, shop).await?;
    }
    Ok(Json(
        super::sessions::issue_session_with_proof(&a, &user, shop, claims.session_token.as_deref())
            .await?,
    ))
}
