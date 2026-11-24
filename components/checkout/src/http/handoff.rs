//! Single-use checkout transfer for independent storefronts; no app-specific catalog or checkout rules.
use crate::store::cart_load;
use crate::store::hosted_parent::registered_parent;
use axum::Json;
use axum::extract::State as Shared;
use axum::http::header::CACHE_CONTROL;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_b2b::store::access::guard_conn;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel, tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, uid};
use veyndo_shop::domain::cart_admission::request;
use veyndo_shop::types::cart::{StoredCart, stored};

/// The JSON fields `issue` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct IssueInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    parent_origin: Option<String>,
}

/// `POST /store-api/checkout/handoff`: a ten-minute single-use ticket for the hosted checkout, with an optional recovery key for a framing storefront.
pub async fn issue(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: IssueInput = view(&v);
    let t = tenant(&h)?;
    let _ = cart_load::load(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    let r = sqlx::query("SELECT * FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE")
        .bind(&t)
        .bind(token(&h)?)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(bad("Cart unavailable"))?;
    let c = stored(&r)?;
    request(&h, &c)?;
    guard_conn(&mut tx, &h, &c).await?;
    if c.status != "open" || c.data.items.is_empty() || input.revision != Some(c.revision) {
        return Err(conflict("Cart changed, empty or terminal"));
    }
    let ticket = uid();
    let parent = input.parent_origin.as_deref();
    let recovery = if let Some(origin) = parent {
        registered_parent(&mut tx, &t, &c.data.sales_channel, origin).await?;
        Some(uid())
    } else {
        None
    };
    sqlx::query("DELETE FROM checkout_handoffs WHERE tenant=$1 AND greatest(expires_at,recovery_expires_at)<now()")
        .bind(&t)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO checkout_handoffs(digest,tenant,cart_id,expires_at,recovery_digest,recovery_origin,recovery_expires_at) VALUES($1,$2,$3,now()+interval '10 minutes',$4::text,$5,CASE WHEN $4::text IS NOT NULL THEN now()+interval '24 hours' END) ON CONFLICT(cart_id) DO UPDATE SET digest=EXCLUDED.digest,expires_at=EXCLUDED.expires_at,recovery_digest=EXCLUDED.recovery_digest,recovery_origin=EXCLUDED.recovery_origin,recovery_expires_at=EXCLUDED.recovery_expires_at,consumed_at=NULL")
        .bind(hash(&ticket)).bind(&t).bind(&c.id).bind(recovery.as_deref().map(hash)).bind(parent).execute(&mut *tx).await?;
    tx.commit().await?;
    let mut result =
        json!({"checkoutPath":checkout_path(&c, &ticket),"expiresInSeconds":600,"singleUse":true});
    if let Some(key) = recovery {
        result["recovery"] = json!({"cartId":c.id,"key":key});
    }
    Ok(Json(result))
}

/// The hosted checkout path; the ticket stays in the fragment and never reaches a server log.
fn checkout_path(c: &StoredCart, ticket: &str) -> String {
    format!(
        "/checkout?shop={}&channel={}&language={}#checkout/{ticket}",
        c.tenant, c.data.sales_channel, c.data.locale
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// A recovery request of the framing storefront.
pub struct Recovery {
    cart_id: String,
    key: String,
    parent_origin: String,
    #[serde(default)]
    reopen: bool,
}

/// The one answer every refused recovery gets, so a key cannot be probed.
fn unavailable() -> Error {
    Error(
        StatusCode::NOT_FOUND,
        "Hosted checkout recovery unavailable".into(),
    )
}

/// `POST /store-api/checkout/handoff/recover`: the framing storefront polls or reopens its checkout with the recovery key.
pub async fn recover(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(input): Json<Recovery>,
) -> Result<impl IntoResponse + use<>> {
    let t = tenant(&h)?;
    let channel = channel(&h);
    if [&input.cart_id, &input.key]
        .iter()
        .any(|s| s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(unavailable());
    }
    let mut tx = a.db.begin().await?;
    registered_parent(&mut tx, &t, channel, &input.parent_origin).await?;
    let digest = hash(&input.key);
    // Status polling can read the last committed state while an order is being placed.
    let mut query = sqlx::QueryBuilder::new(
        "SELECT c.* FROM carts c JOIN checkout_handoffs h ON h.cart_id=c.id AND h.tenant=c.tenant WHERE c.tenant=$1 AND c.id=$2 AND h.recovery_digest=$3 AND h.recovery_origin=$4 AND h.recovery_expires_at>clock_timestamp()",
    );
    if input.reopen {
        query.push(" FOR UPDATE OF c");
    }
    let row = query
        .build()
        .bind(&t)
        .bind(&input.cart_id)
        .bind(&digest)
        .bind(&input.parent_origin)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(unavailable)?;
    let c = stored(&row)?;
    if c.data.sales_channel != channel {
        return Err(unavailable());
    }
    if input.reopen {
        // The recovery key may have changed while this request waited for the cart lock.
        let admitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM checkout_handoffs WHERE tenant=$1 AND cart_id=$2 AND recovery_digest=$3 AND recovery_origin=$4 AND recovery_expires_at>clock_timestamp())")
            .bind(&t).bind(&c.id).bind(&digest).bind(&input.parent_origin).fetch_one(&mut *tx).await?;
        if !admitted {
            return Err(unavailable());
        }
    }
    request(&h, &c)?;
    guard_conn(&mut tx, &h, &c).await?;
    let result = if c.status == "completed" {
        let order = c
            .data
            .order
            .as_ref()
            .and_then(|o| o["id"].as_str())
            .ok_or_else(unavailable)?;
        json!({"state":"ordered","completion":{"cartId":c.id,"token":c.token,"orderId":order}})
    } else if c.status == "open" && input.reopen {
        let ticket = uid();
        // Fence the old frame before allowing a replacement ticket for this same cart.
        sqlx::query("UPDATE carts SET token=$1,revision=revision+1 WHERE tenant=$2 AND id=$3")
            .bind(uid())
            .bind(&t)
            .bind(&c.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE checkout_handoffs SET digest=$1,expires_at=now()+interval '10 minutes',consumed_at=NULL WHERE tenant=$2 AND cart_id=$3")
            .bind(hash(&ticket)).bind(&t).bind(&c.id).execute(&mut *tx).await?;
        json!({"state":"open","checkoutPath":checkout_path(&c, &ticket)})
    } else if c.status == "open" {
        json!({"state":"open"})
    } else {
        return Err(unavailable());
    };
    tx.commit().await?;
    Ok(([(CACHE_CONTROL, "no-store")], Json(result)))
}
/// The JSON fields `consume` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConsumeInput {
    #[serde(deserialize_with = "lenient")]
    ticket: Option<String>,
}

/// Redeems a ticket once: the cart gets a fresh token the hosted checkout continues with.
pub async fn consume(a: &State, h: &RequestContext, v: &Value) -> Result<StoredCart> {
    let input: ConsumeInput = view(v);
    let t = tenant(h)?;
    let ticket = input
        .ticket
        .as_deref()
        .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(bad("Invalid checkout ticket"))?;
    let mut tx = a.db.begin().await?;
    // Lock the cart first, matching issue/order lock order. The secret remains only in the browser fragment.
    let r = sqlx::query("SELECT c.* FROM carts c JOIN checkout_handoffs h ON h.cart_id=c.id AND h.tenant=c.tenant WHERE h.tenant=$1 AND h.digest=$2 AND h.expires_at>now() AND h.consumed_at IS NULL FOR UPDATE OF c")
        .bind(&t).bind(hash(ticket)).fetch_optional(&mut *tx).await?
        .ok_or(Error(StatusCode::GONE,"Checkout ticket expired or already used".into()))?;
    let c = stored(&r)?;
    request(h, &c)?;
    guard_conn(&mut tx, h, &c).await?;
    if c.data.sales_channel != channel(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cart belongs to another sales channel".into(),
        ));
    }
    if c.status != "open" {
        return Err(conflict("Cart is terminal"));
    }
    let consumed = sqlx::query(
        "UPDATE checkout_handoffs SET consumed_at=clock_timestamp() WHERE tenant=$1 AND digest=$2 AND expires_at>clock_timestamp() AND consumed_at IS NULL",
    )
    .bind(&t)
    .bind(hash(ticket))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if consumed != 1 {
        return Err(Error(
            StatusCode::GONE,
            "Checkout ticket already used".into(),
        ));
    }
    let r = sqlx::query(
        "UPDATE carts SET token=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 RETURNING *",
    )
    .bind(uid())
    .bind(&t)
    .bind(&c.id)
    .fetch_one(&mut *tx)
    .await?;
    let c = stored(&r)?;
    request(h, &c)?;
    guard_conn(&mut tx, h, &c).await?;
    tx.commit().await?;
    Ok(c)
}
