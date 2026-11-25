//! Durable withdrawal and data-rights intake, tenant-scoped operator review and customer-held receipt access.
use crate::types::cart::StoredCart;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash};
use veyndo_identity::domain::credentials::email;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Intake {
    kind: String,
    name: String,
    email: String,
    reference: String,
    message: String,
    request_key: String,
}

/// Records a request of the cart's holder; the same request key returns the first receipt.
pub async fn create(a: &State, c: &StoredCart, v: Intake) -> Result<Value> {
    email(&json!({"email": v.email}))?;
    if ![
        "withdrawal",
        "access",
        "erase",
        "correct",
        "portability",
        "objection",
    ]
    .contains(&v.kind.as_str())
        || v.name.trim().is_empty()
        || v.name.len() > 200
        || v.email.len() > 254
        || !v.email.contains('@')
        || v.reference.len() > 200
        || v.message.len() > 6000
        || v.request_key.len() < 8
        || v.request_key.len() > 128
    {
        return Err(bad("Invalid consumer request"));
    }
    if v.kind == "withdrawal" && v.reference.trim().is_empty() {
        return Err(bad("Contract reference required"));
    }
    let id = hash(&format!("{}:{}:{}", c.tenant, c.id, v.request_key));
    let data = json!({"name":v.name,"email":v.email,"reference":v.reference,"message":v.message,"receivedAt":chrono::Utc::now().to_rfc3339(),"identityVerified":false,"automaticRefund":false,"locale":c.data.locale});
    let mut tx = a.db.begin().await?;
    // Cart lock serializes rate admission and retry; a reference is a declaration, never an order-access grant.
    sqlx::query_scalar!(
        "SELECT id FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE",
        c.tenant,
        c.id
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(r) = sqlx::query!(
        "SELECT data,kind,state FROM consumer_requests WHERE tenant=$1 AND id=$2",
        c.tenant,
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        let old: Declaration = view(&r.data);
        if old != view::<Declaration>(&data) || r.kind != v.kind {
            return Err(conflict("Request key reused with different content"));
        }
        return Ok(json!({"id":id,"kind":v.kind,"data":public_data(&r.data),"state":r.state}));
    }
    let count = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM consumer_requests WHERE tenant=$1 AND cart_id=$2 AND created_at>now()-interval '1 day'"#, c.tenant, c.id).fetch_one(&mut *tx).await?;
    if count >= 20 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Consumer request limit reached".into(),
        ));
    }
    sqlx::query!("INSERT INTO consumer_requests(tenant,id,cart_id,channel_id,kind,data) VALUES($1,$2,$3,$4,$5,$6)",
      c.tenant, id, c.id, c.data.sales_channel, v.kind, data).execute(&mut *tx).await?;
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        &c.tenant,
        &format!("consumer.{}.requested", v.kind),
        &json!({"requestId":id,"salesChannelId":c.data.sales_channel,"kind":v.kind,"receipt":data}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"kind":v.kind,"data":data,"state":"received"}))
}

/// The receipt as its holder may read it, bound to the cart and channel that filed it.
pub async fn receipt(a: &State, c: &StoredCart, id: &str) -> Result<Value> {
    let r=sqlx::query!("SELECT kind,data,state FROM consumer_requests WHERE tenant=$1 AND id=$2 AND cart_id=$3 AND channel_id=$4", c.tenant, id, c.id, c.data.sales_channel).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"Request not found".into()))?;
    Ok(json!({"id":id,"kind":r.kind,"data":public_data(&r.data),"state":r.state}))
}

/// The most recent requests of a shop for operator review.
pub async fn list(a: &State, t: &str) -> Result<Value> {
    let rs = sqlx::query_as!(
        ListedRequest,
        "SELECT id,kind,data,state,revision,channel_id FROM consumer_requests WHERE tenant=$1 ORDER BY created_at DESC,id LIMIT 100",
        t
    )
    .fetch_all(&a.db)
    .await?;
    Ok(
        json!({"elements":rs.iter().map(|r|json!({"id":r.id,"kind":r.kind,"data":r.data,"state":r.state,"revision":r.revision,"salesChannelId":r.channel_id})).collect::<Vec<_>>(),"limit":100}),
    )
}

/// One consumer request as the operator list shows it; columns checked at compile time.
struct ListedRequest {
    id: String,
    kind: String,
    data: Value,
    state: String,
    revision: i64,
    channel_id: String,
}

/// The JSON fields `review` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewInput {
    #[serde(deserialize_with = "lenient")]
    state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    note: Option<String>,
}

/// Moves a request to a reviewed state at the expected revision and keeps the reviewer's note.
pub async fn review(a: &State, h: &RequestContext, t: &str, id: &str, v: &Value) -> Result<Value> {
    let input: ReviewInput = view(v);
    let state = input
        .state
        .as_deref()
        .filter(|s| ["in_review", "completed", "declined"].contains(s))
        .ok_or(bad("Invalid consumer request state"))?;
    let revision = input.revision.ok_or(bad("Revision required"))?;
    let note = input
        .note
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 6000)
        .ok_or(bad("Review note required"))?;
    let mut tx = a.db.begin().await?;
    let n=sqlx::query_scalar!("UPDATE consumer_requests SET state=$1,revision=revision+1,data=data||jsonb_build_object('reviewNote',$2::text,'reviewedAt',now()::text,'reviewedBy',$3::text) WHERE tenant=$4 AND id=$5 AND revision=$6 RETURNING revision",
      state, note, header(h,"x-rac-user").unwrap_or("merchant"), t, id, revision).fetch_optional(&mut *tx).await?.ok_or(conflict("Request changed or unavailable"))?;
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        t,
        "consumer.request.reviewed",
        &json!({"requestId":id,"state":state,"revision":n}),
    )
    .await?;
    sqlx::query!("INSERT INTO consumer_request_reviews(tenant,request_id,revision,actor,state,note) VALUES($1,$2,$3,$4,$5,$6)", t, id, revision+1, header(h,"x-rac-user").unwrap_or("merchant"), state, note).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(json!({"revision":revision+1}))
}

/// What the filer declared in a request; a retry with the same key must declare the same.
#[derive(Deserialize, Default, PartialEq)]
#[serde(default)]
struct Declaration {
    name: Value,
    email: Value,
    reference: Value,
    message: Value,
}

/// The receipt fields its holder may read.
#[derive(Deserialize, Serialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PublicData {
    name: Value,
    email: Value,
    reference: Value,
    message: Value,
    received_at: Value,
    identity_verified: Value,
    automatic_refund: Value,
    locale: Value,
}

/// Internal review notes and reviewer identities never leave the merchant boundary.
fn public_data(v: &Value) -> Value {
    json!(view::<PublicData>(v))
}
