//! Reads of the payment attempt ledger as typed `Attempt` records; locks and snapshots stay with the caller's executor. Columns checked at compile time.
use crate::domain::attempt::Attempt;
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;

/// One attempt of the shop.
pub async fn find<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
) -> Result<Option<Attempt>> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE tenant=$1 AND id=$2"#,
        tenant,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// One attempt of the shop that must exist (a missing row is a database error, as `fetch_one` reports it).
pub async fn get<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
) -> Result<Attempt> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE tenant=$1 AND id=$2"#,
        tenant,
        id
    )
    .fetch_one(db)
    .await?)
}

/// One attempt of the shop, row-locked for the rest of the caller's transaction.
pub async fn lock<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
) -> Result<Option<Attempt>> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        tenant,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// One attempt of the shop that the given provider owns.
pub async fn find_for_provider<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
    provider: &str,
) -> Result<Option<Attempt>> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE tenant=$1 AND id=$2 AND provider=$3"#,
        tenant,
        id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

/// One attempt of the shop bound to the given order.
pub async fn find_for_order<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
    order: &str,
) -> Result<Option<Attempt>> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE tenant=$1 AND id=$2 AND order_id=$3"#,
        tenant,
        id,
        order
    )
    .fetch_optional(db)
    .await?)
}

/// The attempt an order currently names as its payment (a missing row is a database error).
pub async fn current_of_order<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    order: &str,
) -> Result<Attempt> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT p.id,p.provider,p.provider_context AS context,p.tenant,p.order_id AS "order",p.amount_minor AS amount,p.currency,p.currency_scale,p.state,p.provider_order,p.capture_id AS capture,p.refunded_minor AS refunded,p.adapter_version,p.environment,p.bn_code FROM payment_attempts p JOIN orders o ON o.tenant=p.tenant AND o.id=p.order_id AND o.data#>>'{payment,attemptId}'=p.id WHERE p.tenant=$1 AND o.id=$2"#,
        tenant,
        order
    )
    .fetch_one(db)
    .await?)
}

/// An attempt by its global id, for the worker that claimed its job (a missing row is a database error).
pub async fn get_claimed<'e>(
    db: impl Executor<'e, Database = Postgres>,
    id: &str,
) -> Result<Attempt> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE id=$1"#,
        id
    )
    .fetch_one(db)
    .await?)
}

/// An attempt by its global id, row-locked before its job (a missing row is a database error).
pub async fn lock_claimed<'e>(
    db: impl Executor<'e, Database = Postgres>,
    id: &str,
) -> Result<Attempt> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE id=$1 FOR UPDATE"#,
        id
    )
    .fetch_one(db)
    .await?)
}

/// The oldest expired, still open attempt of the worker's scope without an active job, locked and skipped by other workers.
pub async fn lock_expired<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenants: &[String],
    providers: &[String],
) -> Result<Option<Attempt>> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts p WHERE (provider='paypal' AND tenant=ANY($1) OR provider=ANY($2)) AND expires_at<now() AND state IN ('pending','ready','approved') AND NOT EXISTS(SELECT 1 FROM payment_jobs j WHERE j.attempt_id=p.id AND j.state IN ('queued','running','uncertain')) ORDER BY expires_at LIMIT 1 FOR UPDATE SKIP LOCKED"#,
        tenants,
        providers
    )
    .fetch_optional(db)
    .await?)
}

/// The oldest attempt of the worker's scope with a due job and no live lease, locked and skipped by other workers.
pub async fn lock_due<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenants: &[String],
    providers: &[String],
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT p.id FROM payment_attempts p WHERE ((p.provider='paypal' AND p.tenant=ANY($1)) OR p.provider=ANY($2)) AND EXISTS(SELECT 1 FROM payment_jobs j WHERE j.tenant=p.tenant AND j.attempt_id=p.id AND j.available_at<=now() AND (j.state='queued' OR j.state='running' AND j.lease_until<now())) AND NOT EXISTS(SELECT 1 FROM payment_jobs j WHERE j.tenant=p.tenant AND j.attempt_id=p.id AND j.state='running' AND j.lease_until>now()) ORDER BY p.created_at LIMIT 1 FOR UPDATE OF p SKIP LOCKED",
        tenants,
        providers
    )
    .fetch_optional(db)
    .await?)
}

/// An attempt with the fields of the merchant overview and the status projection.
pub struct Listed {
    pub attempt: Attempt,
    pub approval_url: Option<String>,
    pub revision: i64,
}

/// One attempt row with its status-only columns.
struct StatusRow {
    id: String,
    provider: String,
    context: Value,
    tenant: String,
    order: String,
    amount: i64,
    currency: String,
    currency_scale: i16,
    state: String,
    provider_order: Option<String>,
    capture: Option<String>,
    refunded: i64,
    adapter_version: String,
    environment: String,
    bn_code: String,
    approval_url: Option<String>,
    revision: i64,
    job: Option<Value>,
    sale: Option<Value>,
}

impl StatusRow {
    fn split(self) -> (Listed, Option<Value>, Option<Value>) {
        (
            Listed {
                attempt: Attempt {
                    id: self.id,
                    provider: self.provider,
                    context: self.context,
                    tenant: self.tenant,
                    order: self.order,
                    amount: self.amount,
                    currency: self.currency,
                    currency_scale: self.currency_scale,
                    state: self.state,
                    provider_order: self.provider_order,
                    capture: self.capture,
                    refunded: self.refunded,
                    adapter_version: self.adapter_version,
                    environment: self.environment,
                    bn_code: self.bn_code,
                },
                approval_url: self.approval_url,
                revision: self.revision,
            },
            self.job,
            self.sale,
        )
    }
}

/// An attempt with its latest job, and the sale (order document) when a cart is given.
pub struct Status {
    pub listed: Listed,
    pub job: Option<Value>,
    pub sale: Option<Value>,
}

// All status components come from one PostgreSQL statement snapshot. A provider
// commit must never combine a new payment with an older admitted cart/order.
/// The status of one attempt, read in one statement; with a cart, only when the attempt's order belongs to that cart.
pub async fn status<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
    cart: Option<&str>,
) -> Result<Option<Status>> {
    let row = if let Some(cart) = cart {
        sqlx::query_as!(
            StatusRow,
            r#"SELECT p.id,p.provider,p.provider_context AS context,p.tenant,p.order_id AS "order",p.amount_minor AS amount,p.currency,p.currency_scale,p.state,p.provider_order,p.capture_id AS capture,p.refunded_minor AS refunded,p.adapter_version,p.environment,p.bn_code,p.approval_url,p.revision,(SELECT jsonb_build_object('id',j.id,'operation',j.operation,'state',j.state) FROM payment_jobs j WHERE j.tenant=p.tenant AND j.attempt_id=p.id ORDER BY j.created_at DESC,j.id DESC LIMIT 1) AS job,o.data AS "sale?" FROM payment_attempts p JOIN orders o ON o.tenant=p.tenant AND o.id=p.order_id WHERE p.tenant=$1 AND p.id=$2 AND o.cart_id=$3"#,
            tenant,
            id,
            cart
        )
        .fetch_optional(db)
        .await?
    } else {
        sqlx::query_as!(
            StatusRow,
            r#"SELECT p.id,p.provider,p.provider_context AS context,p.tenant,p.order_id AS "order",p.amount_minor AS amount,p.currency,p.currency_scale,p.state,p.provider_order,p.capture_id AS capture,p.refunded_minor AS refunded,p.adapter_version,p.environment,p.bn_code,p.approval_url,p.revision,(SELECT jsonb_build_object('id',j.id,'operation',j.operation,'state',j.state) FROM payment_jobs j WHERE j.tenant=p.tenant AND j.attempt_id=p.id ORDER BY j.created_at DESC,j.id DESC LIMIT 1) AS job,NULL::jsonb AS sale FROM payment_attempts p WHERE p.tenant=$1 AND p.id=$2"#,
            tenant,
            id
        )
        .fetch_optional(db)
        .await?
    };
    Ok(row.map(|r| {
        let (listed, job, sale) = r.split();
        Status { listed, job, sale }
    }))
}

/// The shop's latest 100 attempts, newest first.
pub async fn latest<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Vec<Attempt>> {
    Ok(sqlx::query_as!(
        Attempt,
        r#"SELECT id,provider,provider_context AS context,tenant,order_id AS "order",amount_minor AS amount,currency,currency_scale,state,provider_order,capture_id AS capture,refunded_minor AS refunded,adapter_version,environment,bn_code FROM payment_attempts WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100"#,
        tenant
    )
    .fetch_all(db)
    .await?)
}

/// The ledger fields a verified provider receipt sets on its attempt.
pub struct Receipt<'a> {
    pub state: &'a str,
    pub provider_order: &'a str,
    /// Keeps the stored approval URL when the receipt has none.
    pub approval_url: Option<&'a str>,
    pub capture: Option<&'a str>,
    pub refunded: i64,
    pub context: &'a Value,
}

/// Applies a verified receipt to the attempt and advances its revision.
pub async fn record_receipt(tx: &mut PgConnection, p: &Attempt, r: Receipt<'_>) -> Result<()> {
    sqlx::query!("UPDATE payment_attempts SET state=$1,provider_order=$2,approval_url=coalesce($3,approval_url),capture_id=$4,refunded_minor=$5,provider_context=$6,revision=revision+1 WHERE tenant=$7 AND id=$8", r.state, r.provider_order, r.approval_url, r.capture, r.refunded, r.context, p.tenant, p.id).execute(tx).await?;
    Ok(())
}
