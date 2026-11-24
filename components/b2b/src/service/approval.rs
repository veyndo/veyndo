//! Revision-bound purchasing approval inbox; approvers must be active peers and cannot approve themselves.
use crate::store::access::{access, catalog_guard, guard_conn, identity, require};
use crate::store::purchase;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::history_context as history;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, uid};
use veyndo_kernel::verified_kernel;
use veyndo_shop::ports::Carts;
/// The JSON fields `request` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RequestInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

pub async fn request(carts: &dyn Carts, a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: RequestInput = view(v);
    let c = carts.load(h).await?;
    let context = c
        .data
        .b2b
        .as_ref()
        .ok_or(bad("Select a company unit first"))?;
    if c.status != "open" || c.data.items.is_empty() {
        return Err(bad("Open nonempty cart required"));
    }
    if input.revision != Some(c.revision) {
        return Err(conflict("Cart changed; reload before requesting approval"));
    }
    let mut q = carts.view(&c).await?;
    if let Some(snapshot) = q.as_object_mut() {
        snapshot.remove("token");
    }
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, h, "b2b.procurement").await?;
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM carts WHERE tenant=$1 AND id=$2 AND status='open' FOR UPDATE",
    )
    .bind(&c.tenant)
    .bind(&c.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(conflict("Cart is terminal"))?;
    if revision != c.revision {
        return Err(conflict("Cart changed; reload before requesting approval"));
    }
    let policy = guard_conn(&mut tx, h, &c)
        .await?
        .ok_or(bad("Company context required"))?;
    require(&policy, "buy")?;
    history::customer_context(&mut tx, &context.customer_id).await?;
    catalog_guard(&mut tx, &c, &policy).await?;
    let digest = purchase::fingerprint(&c, &q, &policy.version);
    let id = uid();
    let data = json!({"quote":q,"buyerId":context.customer_id,"unitId":context.unit_id,"policyVersion":policy.version});
    sqlx::query("INSERT INTO b2b_approvals(tenant,id,unit_id,buyer_id,cart_id,fingerprint,data) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(tenant,cart_id,fingerprint) DO NOTHING").bind(&c.tenant).bind(&id).bind(&context.unit_id).bind(&context.customer_id).bind(&c.id).bind(&digest).bind(data).execute(&mut *tx).await?;
    let r=sqlx::query("SELECT id,state,revision FROM b2b_approvals WHERE tenant=$1 AND cart_id=$2 AND fingerprint=$3").bind(&c.tenant).bind(&c.id).bind(digest).fetch_one(&mut *tx).await?;
    let actual: String = r.get("id");
    if actual == id {
        crate::store::organization::event(
            &mut tx,
            &c.tenant,
            "b2b.approval_requested",
            json!({"approvalId":id,"unitId":context.unit_id}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(
        json!({"id":actual,"state":r.get::<String,_>("state"),"revision":r.get::<i64,_>("revision")}),
    )
}
/// The JSON fields `inbox` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InboxInput {
    #[serde(deserialize_with = "lenient")]
    unit_id: Option<String>,
}

pub async fn inbox(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: InboxInput = view(v);
    let t = tenant(h)?;
    let unit = input.unit_id.as_deref().ok_or(bad("Unit required"))?;
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, h, "b2b.procurement").await?;
    let customer = identity(&mut tx, h).await?;
    history::customer_context(&mut tx, &customer).await?;
    let policy = access(&mut tx, &t, unit, &customer).await?;
    let approver = policy.member.allows("approve");
    let rows=sqlx::query("SELECT id,buyer_id,state,data,revision,created_at::text AS created FROM b2b_approvals WHERE tenant=$1 AND unit_id=$2 AND ($3 OR buyer_id=$4) ORDER BY created_at DESC LIMIT 100").bind(&t).bind(unit).bind(approver).bind(customer).fetch_all(&mut *tx).await?;
    Ok(
        json!({"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"buyerId":r.get::<String,_>("buyer_id"),"state":r.get::<String,_>("state"),"data":r.get::<Value,_>("data"),"revision":r.get::<i64,_>("revision"),"createdAt":r.get::<String,_>("created")})).collect::<Vec<_>>()}),
    )
}
/// The JSON fields `decide` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DecideInput {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

pub async fn decide(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: DecideInput = view(v);
    let t = tenant(h)?;
    let id = input.id.as_deref().ok_or(bad("Approval required"))?;
    let target = input
        .state
        .as_deref()
        .filter(|s| ["approved", "rejected"].contains(s))
        .ok_or(bad("Approve or reject required"))?;
    let revision = input.revision.ok_or(bad("Approval revision required"))?;
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, h, "b2b.procurement").await?;
    let customer = identity(&mut tx, h).await?;
    history::customer_context(&mut tx, &customer).await?;
    // Unit first matches checkout's lock order; no approval->unit deadlock.
    let unit: Option<String> =
        sqlx::query_scalar("SELECT unit_id FROM b2b_approvals WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let unit = unit.ok_or(Error(StatusCode::NOT_FOUND, "Approval not found".into()))?;
    sqlx::query("SELECT id FROM b2b_units WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&t)
        .bind(&unit)
        .fetch_one(&mut *tx)
        .await?;
    let policy = access(&mut tx, &t, &unit, &customer).await?;
    require(&policy, "approve")?;
    let r = sqlx::query(
        "SELECT buyer_id,state,revision FROM b2b_approvals WHERE tenant=$1 AND id=$2 FOR UPDATE",
    )
    .bind(&t)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if !verified_kernel::b2b_approval_admissible(
        r.get::<String, _>("buyer_id") != customer,
        true,
        r.get::<String, _>("state") == "pending",
        r.get::<i64, _>("revision") == revision,
    ) {
        return Err(conflict(
            "Approval requires a different approver and a current pending revision",
        ));
    }
    sqlx::query("UPDATE b2b_approvals SET state=$3,revision=revision+1,data=data||jsonb_build_object('approverId',$4::text,'decidedAt',now()::text) WHERE tenant=$1 AND id=$2").bind(&t).bind(id).bind(target).bind(customer).execute(&mut *tx).await?;
    crate::store::organization::event(
        &mut tx,
        &t,
        "b2b.approval_decided",
        json!({"approvalId":id,"unitId":unit,"state":target}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"state":target,"revision":revision+1}))
}
