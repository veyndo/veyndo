//! Persisted storefront layout policy, consent-bound session personalization and observed synthetic rewards.
use super::preferences::forget;
use crate::service::consent::{require, require_locked};
use crate::store::consent::forget_personalization as forget_signals;
use crate::store::experiments;
use crate::store::personalization as store;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veyndo_catalog::ports::Catalog;
use veyndo_catalog::service::catalog_reads::page;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, hash};
use veyndo_identity::merchant;
use veyndo_marketing::service::channels::{admit_product, catalog_scope};
use veyndo_pricing::catalog::Product;
use veyndo_shop::service::language::language_context;

/// The storefront layout variant of the cart's session: a running experiment's assignment, else a persisted epsilon-greedy choice; static without consent.
pub async fn experience(
    a: &State,
    secret: &str,
    h: RequestContext,
    v: Value,
) -> Result<Json<Value>> {
    let t = tenant(&h)?;
    let Ok(c) = load_cart(a, &h).await else {
        return Ok(Json(
            json!({"variant":"discovery","propensity":1.0,"adaptation":{"localBehavior":false,"policy":"no session; static layout"}}),
        ));
    };
    if require(a, &c, "personalization").await.is_err() {
        return Ok(Json(
            json!({"variant":"discovery","propensity":1.0,"adaptation":{"localBehavior":false,"policy":"no consent; static layout"}}),
        ));
    }
    let session = view::<ExperienceInput>(&v).session;
    let session = session
        .as_deref()
        .filter(|s| s.len() >= 8 && s.len() <= 128)
        .ok_or(bad("Session ID required"))?;
    if session != c.data.session {
        return Err(bad("Experience session must match cart"));
    }
    let requested_locale = language_context(a, &h).await?.0;
    let er = store::experience(&a.db, &t).await?;
    let e: ExperienceDocument = view(&er.data);
    let mut tx = a.db.begin().await?;
    require_locked(&mut tx, &c, "personalization").await?;
    let subject = experiments::Subject {
        tenant: &c.tenant,
        cart: &c.id,
        channel: &c.data.sales_channel,
        open: c.status == "open",
        currency: &c.data.currency,
    };
    if let Some(assignment) = experiments::assign(&mut tx, secret, subject).await? {
        tx.commit().await?;
        return Ok(Json(assignment));
    }
    let existing = store::exposure(&mut *tx, &t, session).await?;
    let (variant, probability) = if let Some(r) = existing {
        (r.variant, r.propensity)
    } else {
        let rows = store::variant_stats(&mut *tx, &t).await?;
        let best = rows
            .iter()
            .max_by(|x, y| {
                let rate =
                    |r: &store::VariantStats| (r.purchases as f64 + 1.) / (r.views as f64 + 2.);
                rate(x).total_cmp(&rate(y))
            })
            .map(|r| r.variant.clone())
            .unwrap_or("discovery".into());
        let digest = Sha256::digest(session.as_bytes());
        let explore = digest[0] < 51;
        let random = if digest[1] % 2 == 0 {
            "discovery"
        } else {
            "comparison"
        };
        let chosen = if e.mode == "balanced" {
            if explore {
                random.to_string()
            } else {
                best.clone()
            }
        } else {
            e.mode.as_str().unwrap().to_string()
        };
        let epsilon = 51.0 / 256.0;
        let p = if e.mode != "balanced" {
            1.
        } else if chosen == best {
            1.0 - epsilon / 2.0
        } else {
            epsilon / 2.0
        };
        store::expose(&mut tx, &t, session, &chosen, p).await?;
        let saved = store::exposure(&mut *tx, &t, session)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        (saved.variant, saved.propensity)
    };
    tx.commit().await?;
    Ok(Json(
        json!({"schemaVersion":1,"revision":er.revision,"variant":variant,"propensity":probability,"headline":if e.headline_locale.as_str().unwrap_or("en-GB")==requested_locale{e.headline}else{Value::Null},"blocks":[{"type":"hero"},{"type":if variant=="comparison"{"comparison-grid"}else{"product-grid"}},{"type":"cart"}],"adaptation":{"localBehavior":true,"policy":"persisted epsilon-greedy; simulated purchase reward"}}),
    ))
}
/// The observed views and purchases of each layout variant.
pub async fn policy_stats(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let rs = store::variant_stats(&a.db, &t).await?;
    Ok(Json(
        json!({"variants":rs.iter().map(|r|json!({"variant":r.variant,"views":r.views,"purchases":r.purchases})).collect::<Vec<_>>()}),
    ))
}

/// The request fields `experience` reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ExperienceInput {
    #[serde(deserialize_with = "lenient")]
    session: Option<String>,
}

/// The fields of the shop's experience document the layout policy reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ExperienceDocument {
    mode: Value,
    headline_locale: Value,
    headline: Value,
}

/// The JSON fields `personalization` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PersonalizationInput {
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    event_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
}

/// Product behavior updates a tenant/session profile; the response immediately ranks available channel products.
pub async fn personalization(
    Shared(catalog): Shared<Catalog>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: PersonalizationInput = view(&v);
    let a = catalog.core.clone();
    let c = load_cart(&a, &h).await?;
    require(&a, &c, "personalization").await?;
    let id = input.product_id.as_deref().ok_or(bad("Product required"))?;
    let event = input
        .event_id
        .as_deref()
        .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(bad("Event ID required"))?;
    let kind = input
        .kind
        .as_deref()
        .filter(|s| ["view", "cart_add"].contains(s))
        .ok_or(bad("Unsupported behavior signal"))?;
    let exists = store::product_exists(&a.db, &c.tenant, id).await?;
    if !exists {
        return Err(bad("Unknown product"));
    }
    admit_product(&a, &h, id).await?;

    if kind == "cart_add" && !c.data.items.iter().any(|i| i.id == id) {
        return Err(bad("Cart signal must refer to an actual cart item"));
    }
    let session = hash(&c.id);
    let mut tx = a.db.begin().await?;
    require_locked(&mut tx, &c, "personalization").await?;
    store::lock_session(&mut *tx, &format!("{}:{session}", c.tenant)).await?;
    let count = store::recent_events(&mut *tx, &c.tenant, &session).await?;
    if count >= 500 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Behavior signal limit reached".into(),
        ));
    }
    let stored = store::signal(
        &mut tx,
        &c.tenant,
        event,
        &session,
        id,
        i32::from(kind == "view"),
        i32::from(kind == "cart_add"),
    )
    .await?;
    let rows = store::affinities(&mut *tx, &c.tenant, &session).await?;
    tx.commit().await?;
    let (_, chain) = language_context(&a, &h).await?;
    let preferred = rows
        .iter()
        .max_by_key(|r| r.score)
        .map(|r| r.category.clone());
    let scope = catalog_scope(&a, &h).await?;
    let criteria = CatalogCriteria {
        category: preferred,
        product_ids: scope.clone(),
        ..Default::default()
    };
    let mut ps = page(&catalog, &c.tenant, &chain, &criteria).await?.products;
    let all = page(
        &catalog,
        &c.tenant,
        &chain,
        &CatalogCriteria {
            product_ids: scope,
            ..Default::default()
        },
    )
    .await?;
    for p in all.products {
        if !ps.iter().any(|v| v.id == p.id) {
            ps.push(p);
        }
    }
    ps.retain(|p| p.stock > 0);
    let score = |p: &Product| {
        rows.iter()
            .find(|r| r.category == p.category)
            .map(|r| r.score)
            .unwrap_or(0)
    };
    ps.sort_by(|x, y| score(y).cmp(&score(x)).then(x.id.cmp(&y.id)));
    Ok(Json(
        json!({"rankedProductIds":ps.iter().map(|p|&p.id).collect::<Vec<_>>(),"adapted":rows.iter().any(|r|r.score>=3),"basis":"observed-session-category-affinity","explanation":{"signals":["view","cart_add"],"cartWeight":3,"windowDays":30,"ownSessionOnly":true,"changesPrices":false,"categories":rows.iter().map(|r|json!({"category":r.category,"score":r.score})).collect::<Vec<_>>()},"stored":stored,"candidateLimit":100,"causalUpliftProven":false}),
    ))
}
/// Clear this anonymous shop session's behavior and allow the shopper to turn adaptation off.
/// Forgets the session profile and preference graph of the cart.
pub async fn forget_personalization(
    Shared(a): Shared<State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    forget_signals(&mut tx, &c.tenant, &[hash(&c.id)]).await?;
    forget(&mut tx, &c).await?;
    tx.commit().await?;
    Ok(Json(json!({"forgotten":true})))
}
