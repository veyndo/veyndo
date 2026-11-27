//! Revision-bound purchasing approval inbox; approvers must be active peers and cannot approve themselves.
use crate::store::access::{access, catalog_guard, guard_conn, identity, require};
use crate::store::purchase;
use crate::store::{approvals, carts as cart_rows};
use crate::types::events::{ApprovalDecided, ApprovalRequested};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::store::history_context as history;
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
    let revision = cart_rows::open_revision(&mut tx, &c.tenant, &c.id)
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
    let request = approvals::NewApproval {
        tenant: &c.tenant,
        id: &id,
        unit: &context.unit_id,
        buyer: &context.customer_id,
        cart: &c.id,
        fingerprint: &digest,
        data,
    };
    approvals::request(&mut tx, request).await?;
    let r = approvals::by_fingerprint(&mut tx, &c.tenant, &c.id, &digest).await?;
    let actual = r.id;
    if actual == id {
        let requested = ApprovalRequested {
            approval_id: &id,
            unit_id: &context.unit_id,
        };
        crate::store::organization::event(&mut tx, &c.tenant, &requested).await?;
    }
    tx.commit().await?;
    Ok(json!({"id":actual,"state":r.state,"revision":r.revision}))
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
    let rows = approvals::inbox(&mut tx, &t, unit, approver, &customer).await?;
    Ok(
        json!({"elements":rows.into_iter().map(|r|json!({"id":r.id,"buyerId":r.buyer_id,"state":r.state,"data":r.data,"revision":r.revision,"createdAt":r.created})).collect::<Vec<_>>()}),
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
    let unit = approvals::unit_of(&mut tx, &t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Approval not found".into()))?;
    approvals::lock_unit(&mut tx, &t, &unit).await?;
    let policy = access(&mut tx, &t, &unit, &customer).await?;
    require(&policy, "approve")?;
    let r = approvals::locked(&mut tx, &t, id).await?;
    if !verified_kernel::b2b_approval_admissible(
        r.buyer_id != customer,
        true,
        r.state == "pending",
        r.revision == revision,
    ) {
        return Err(conflict(
            "Approval requires a different approver and a current pending revision",
        ));
    }
    approvals::decide(&mut tx, &t, id, target, &customer).await?;
    let decided = ApprovalDecided {
        approval_id: id,
        unit_id: &unit,
        state: target,
    };
    crate::store::organization::event(&mut tx, &t, &decided).await?;
    tx.commit().await?;
    Ok(json!({"id":id,"state":target,"revision":revision+1}))
}
