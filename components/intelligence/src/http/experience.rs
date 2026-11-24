//! Persisted storefront layout policy, consent-bound session personalization and observed synthetic rewards.
use super::preferences::forget;
use crate::service::consent::{require, require_locked};
use crate::store::experiments;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
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
    let session = v["session"]
        .as_str()
        .filter(|s| s.len() >= 8 && s.len() <= 128)
        .ok_or(bad("Session ID required"))?;
    if session != c.data.session {
        return Err(bad("Experience session must match cart"));
    }
    let requested_locale = language_context(a, &h).await?.0;
    let er = sqlx::query("SELECT data,revision FROM experiences WHERE tenant=$1")
        .bind(&t)
        .fetch_one(&a.db)
        .await?;
    let e: Value = er.get("data");
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
    let existing =
        sqlx::query("SELECT variant,propensity FROM exposures WHERE tenant=$1 AND session=$2")
            .bind(&t)
            .bind(session)
            .fetch_optional(&mut *tx)
            .await?;
    let (variant, probability) = if let Some(r) = existing {
        (r.get::<String, _>("variant"), r.get::<f64, _>("propensity"))
    } else {
        let rows = sqlx::query(
            "SELECT variant,views,purchases FROM policy WHERE tenant=$1 ORDER BY variant",
        )
        .bind(&t)
        .fetch_all(&mut *tx)
        .await?;
        let best = rows
            .iter()
            .max_by(|x, y| {
                let rate = |r: &sqlx::postgres::PgRow| {
                    (r.get::<i64, _>("purchases") as f64 + 1.)
                        / (r.get::<i64, _>("views") as f64 + 2.)
                };
                rate(x).total_cmp(&rate(y))
            })
            .map(|r| r.get::<String, _>("variant"))
            .unwrap_or("discovery".into());
        let digest = Sha256::digest(session.as_bytes());
        let explore = digest[0] < 51;
        let random = if digest[1] % 2 == 0 {
            "discovery"
        } else {
            "comparison"
        };
        let chosen = if e["mode"] == "balanced" {
            if explore {
                random.to_string()
            } else {
                best.clone()
            }
        } else {
            e["mode"].as_str().unwrap().to_string()
        };
        let epsilon = 51.0 / 256.0;
        let p = if e["mode"] != "balanced" {
            1.
        } else if chosen == best {
            1.0 - epsilon / 2.0
        } else {
            epsilon / 2.0
        };
        let n=sqlx::query("INSERT INTO exposures(tenant,session,variant,propensity) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(&t).bind(session).bind(&chosen).bind(p).execute(&mut *tx).await?.rows_affected();
        if n == 1 {
            sqlx::query("UPDATE policy SET views=views+1 WHERE tenant=$1 AND variant=$2")
                .bind(&t)
                .bind(&chosen)
                .execute(&mut *tx)
                .await?;
        }
        let saved =
            sqlx::query("SELECT variant,propensity FROM exposures WHERE tenant=$1 AND session=$2")
                .bind(&t)
                .bind(session)
                .fetch_one(&mut *tx)
                .await?;
        (saved.get("variant"), saved.get("propensity"))
    };
    tx.commit().await?;
    Ok(Json(
        json!({"schemaVersion":1,"revision":er.get::<i64,_>("revision"),"variant":variant,"propensity":probability,"headline":if e["headlineLocale"].as_str().unwrap_or("en-GB")==requested_locale{e["headline"].clone()}else{Value::Null},"blocks":[{"type":"hero"},{"type":if variant=="comparison"{"comparison-grid"}else{"product-grid"}},{"type":"cart"}],"adaptation":{"localBehavior":true,"policy":"persisted epsilon-greedy; simulated purchase reward"}}),
    ))
}
/// The observed views and purchases of each layout variant.
pub async fn policy_stats(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let rs = sqlx::query("SELECT * FROM policy WHERE tenant=$1 ORDER BY variant")
        .bind(t)
        .fetch_all(&a.db)
        .await?;
    Ok(Json(
        json!({"variants":rs.iter().map(|r|json!({"variant":r.get::<String,_>("variant"),"views":r.get::<i64,_>("views"),"purchases":r.get::<i64,_>("purchases")})).collect::<Vec<_>>()}),
    ))
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
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)")
            .bind(&c.tenant)
            .bind(id)
            .fetch_one(&a.db)
            .await?;
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
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,18))")
        .bind(format!("{}:{session}", c.tenant))
        .execute(&mut *tx)
        .await?;
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM session_signal_events WHERE tenant=$1 AND session=$2 AND created_at>now()-interval '1 hour'").bind(&c.tenant).bind(&session).fetch_one(&mut *tx).await?;
    if count >= 500 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Behavior signal limit reached".into(),
        ));
    }
    let n=sqlx::query("INSERT INTO session_signal_events(tenant,id,session) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(&c.tenant).bind(event).bind(&session).execute(&mut *tx).await?.rows_affected();
    if n == 1 {
        sqlx::query("INSERT INTO session_signals(tenant,session,product_id,views,cart_adds) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,session,product_id) DO UPDATE SET views=least(session_signals.views+EXCLUDED.views,1000),cart_adds=least(session_signals.cart_adds+EXCLUDED.cart_adds,1000),updated_at=now()").bind(&c.tenant).bind(&session).bind(id).bind(i32::from(kind=="view")).bind(i32::from(kind=="cart_add")).execute(&mut *tx).await?;
    }
    let rows=sqlx::query("SELECT p.category,sum(s.views+s.cart_adds*3)::bigint AS score FROM session_signals s JOIN products p ON p.tenant=s.tenant AND p.id=s.product_id WHERE s.tenant=$1 AND s.session=$2 AND s.updated_at>now()-interval '30 days' GROUP BY p.category").bind(&c.tenant).bind(&session).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let (_, chain) = language_context(&a, &h).await?;
    let preferred = rows
        .iter()
        .max_by_key(|r| r.get::<i64, _>("score"))
        .map(|r| r.get::<String, _>("category"));
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
            .find(|r| r.get::<String, _>("category") == p.category)
            .map(|r| r.get::<i64, _>("score"))
            .unwrap_or(0)
    };
    ps.sort_by(|x, y| score(y).cmp(&score(x)).then(x.id.cmp(&y.id)));
    Ok(Json(
        json!({"rankedProductIds":ps.iter().map(|p|&p.id).collect::<Vec<_>>(),"adapted":rows.iter().any(|r|r.get::<i64,_>("score")>=3),"basis":"observed-session-category-affinity","explanation":{"signals":["view","cart_add"],"cartWeight":3,"windowDays":30,"ownSessionOnly":true,"changesPrices":false,"categories":rows.iter().map(|r|json!({"category":r.get::<String,_>("category"),"score":r.get::<i64,_>("score")})).collect::<Vec<_>>()},"stored":n==1,"candidateLimit":100,"causalUpliftProven":false}),
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
    sqlx::query("DELETE FROM session_signals WHERE tenant=$1 AND session=$2")
        .bind(&c.tenant)
        .bind(hash(&c.id))
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM session_signal_events WHERE tenant=$1 AND session=$2 AND purpose='personalization'")
        .bind(&c.tenant)
        .bind(hash(&c.id))
        .execute(&mut *tx)
        .await?;
    forget(&mut tx, &c).await?;
    tx.commit().await?;
    Ok(Json(json!({"forgotten":true})))
}
