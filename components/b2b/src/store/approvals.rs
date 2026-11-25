//! Spending approvals of company units: one per cart and quote fingerprint, listed for buyers and approvers, decided once at a revision under the unit lock. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// A request for approval of one cart quote.
pub struct NewApproval<'a> {
    pub tenant: &'a str,
    pub id: &'a str,
    pub unit: &'a str,
    pub buyer: &'a str,
    pub cart: &'a str,
    pub fingerprint: &'a str,
    pub data: Value,
}

/// Records the request unless the same cart and fingerprint already have one.
pub async fn request(conn: &mut PgConnection, new: NewApproval<'_>) -> Result<()> {
    sqlx::query!("INSERT INTO b2b_approvals(tenant,id,unit_id,buyer_id,cart_id,fingerprint,data) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(tenant,cart_id,fingerprint) DO NOTHING",
        new.tenant, new.id, new.unit, new.buyer, new.cart, new.fingerprint, new.data)
    .execute(conn)
    .await?;
    Ok(())
}

/// An approval's identity, state and revision.
pub struct ApprovalState {
    pub id: String,
    pub state: String,
    pub revision: i64,
}

/// The approval of a cart for one quote fingerprint.
pub async fn by_fingerprint(
    conn: &mut PgConnection,
    tenant: &str,
    cart: &str,
    fingerprint: &str,
) -> Result<ApprovalState> {
    Ok(sqlx::query_as!(ApprovalState, "SELECT id,state,revision FROM b2b_approvals WHERE tenant=$1 AND cart_id=$2 AND fingerprint=$3", tenant, cart, fingerprint)
        .fetch_one(conn)
        .await?)
}

/// An approval as the inbox lists it.
pub struct InboxEntry {
    pub id: String,
    pub buyer_id: String,
    pub state: String,
    pub data: Value,
    pub revision: i64,
    pub created: String,
}

/// The unit's approvals, all of them for an approver and the buyer's own otherwise, newest first (at most 100).
pub async fn inbox(
    conn: &mut PgConnection,
    tenant: &str,
    unit: &str,
    approver: bool,
    buyer: &str,
) -> Result<Vec<InboxEntry>> {
    Ok(sqlx::query_as!(InboxEntry, r#"SELECT id,buyer_id,state,data,revision,created_at::text AS "created!" FROM b2b_approvals WHERE tenant=$1 AND unit_id=$2 AND ($3 OR buyer_id=$4) ORDER BY created_at DESC LIMIT 100"#,
        tenant, unit, approver, buyer)
    .fetch_all(conn)
    .await?)
}

/// The unit an approval belongs to.
pub async fn unit_of(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT unit_id FROM b2b_approvals WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Locks the unit row; checkout takes the same lock before an approval.
pub async fn lock_unit(conn: &mut PgConnection, tenant: &str, unit: &str) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT id FROM b2b_units WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        unit
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// An approval locked for its decision.
pub struct Pending {
    pub buyer_id: String,
    pub state: String,
    pub revision: i64,
}

/// Approval `id`, locked for this transaction.
pub async fn locked(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Pending> {
    Ok(sqlx::query_as!(
        Pending,
        "SELECT buyer_id,state,revision FROM b2b_approvals WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(conn)
    .await?)
}

/// Records the decision `state` by `approver` and advances the revision.
pub async fn decide(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    state: &str,
    approver: &str,
) -> Result<()> {
    sqlx::query!("UPDATE b2b_approvals SET state=$3,revision=revision+1,data=data||jsonb_build_object('approverId',$4::text,'decidedAt',now()::text) WHERE tenant=$1 AND id=$2",
        tenant, id, state, approver)
    .execute(conn)
    .await?;
    Ok(())
}
