//! Payment status reads from one statement snapshot, and the merchant's view of a durable payment job.
use crate::domain::settlements::summary;
use crate::store::attempts::attempt;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_identity::{merchant, permit};

// All status components come from one PostgreSQL statement snapshot. A provider
// commit must never combine a new payment with an older admitted cart/order.
const STATUS_FIELDS: &str = "SELECT p.*,(SELECT jsonb_build_object('id',j.id,'operation',j.operation,'state',j.state) FROM payment_jobs j WHERE j.tenant=p.tenant AND j.attempt_id=p.id ORDER BY j.created_at DESC,j.id DESC LIMIT 1) AS job";
/// One attempt with its latest job, and the sale when a cart is given.
async fn status_row(
    a: &State,
    t: &str,
    id: &str,
    cart: Option<&str>,
) -> Result<sqlx::postgres::PgRow> {
    let mut q = sqlx::QueryBuilder::<sqlx::Postgres>::new(STATUS_FIELDS);
    if cart.is_some() {
        q.push(",o.data AS sale FROM payment_attempts p JOIN orders o ON o.tenant=p.tenant AND o.id=p.order_id");
    } else {
        q.push(" FROM payment_attempts p");
    }
    q.push(" WHERE p.tenant=")
        .push_bind(t)
        .push(" AND p.id=")
        .push_bind(id);
    if let Some(cart) = cart {
        q.push(" AND o.cart_id=").push_bind(cart);
    }
    q.build()
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))
}
/// The status projection of an attempt row.
fn payment_status(r: &sqlx::postgres::PgRow, id: &str) -> Value {
    let settled = summary(&attempt(r));
    json!({"provider":r.get::<String,_>("provider"),"capturedMinor":settled["capturedMinor"],"captureClosed":settled["captureClosed"],"checkout":r.get::<Value,_>("provider_context")["checkout"],"intent":r.get::<Value,_>("provider_context")["intent"],"job":r.get::<Option<Value>,_>("job"),"id":id,"state":r.get::<String,_>("state"),"approvalUrl":r.get::<Option<String>,_>("approval_url"),"amountMinor":r.get::<i64,_>("amount_minor"),"currency":r.get::<String,_>("currency"),"currencyScale":r.get::<i16,_>("currency_scale"),"refundedMinor":r.get::<i64,_>("refunded_minor"),"revision":r.get::<i64,_>("revision"),"environment":r.get::<String,_>("environment"),"savedPaymentMethodStatus":r.get::<Value,_>("provider_context")["vaultStatus"],"instructionsAvailable":r.get::<Value,_>("provider_context")["capabilities"].as_array().is_some_and(|cs|cs.iter().any(|c|c=="instructions")),"realMoneyCharged":r.get::<String,_>("environment")=="live" && ["captured","partially_captured","captured_late","partially_refunded","refunded"].contains(&r.get::<String,_>("state").as_str())})
}
/// The status of one payment attempt.
pub async fn read(a: &State, t: &str, id: &str) -> Result<Value> {
    Ok(payment_status(&status_row(a, t, id, None).await?, id))
}
/// Caller must first admit the exact cashier/cart capability through the native POS owner.
pub async fn read_sale(a: &State, t: &str, id: &str, cart: &str) -> Result<Value> {
    let r = status_row(a, t, id, Some(cart)).await?;
    Ok(json!({"payment":payment_status(&r,id),"order":r.get::<Value,_>("sale")}))
}

/// Merchant polling observes the durable job instead of dispatching another financial command.
pub async fn job_status(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "payments.manage")?;
    let r =
        sqlx::query("SELECT id,state,error,attempt_id FROM payment_jobs WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Payment job not found".into()))?;
    Ok(Json(
        json!({"id":r.get::<String,_>("id"),"state":r.get::<String,_>("state"),"error":r.get::<Option<String>,_>("error"),"payment":read(&a,&t,&r.get::<String,_>("attempt_id")).await?}),
    ))
}
