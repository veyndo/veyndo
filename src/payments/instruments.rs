//! Customer-owned saved instruments expose only redacted metadata; immediate revocation and provider cleanup share payment jobs.
use crate::*;
pub(crate) async fn identity(a: &App, h: &RequestContext) -> Result<(String, String)> {
    let (t, email) = accounts::identity(a, h).await?;
    let id: String =
        sqlx::query_scalar("SELECT id FROM customers WHERE tenant=$1 AND email=$2 AND active")
            .bind(&t)
            .bind(email)
            .fetch_one(&a.db)
            .await?;
    Ok((t, id))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    if name == "checkout.payment.methods" {
        list(a, h).await
    } else {
        if v["approve"] != true {
            return Err(bad("Approve saved payment revocation"));
        }
        revoke(
            a,
            h,
            v["id"].as_str().ok_or(bad("id required"))?,
            v["requestKey"].as_str().ok_or(bad("requestKey required"))?,
        )
        .await
    }
}
pub(crate) async fn list(a: &App, h: &RequestContext) -> Result<Value> {
    let (t, customer) = identity(a, h).await?;
    let rows=sqlx::query("SELECT id,provider,method,environment,metadata,created_at::text FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND state='active' ORDER BY created_at DESC LIMIT 32").bind(&t).bind(customer).fetch_all(&a.db).await?;
    let manifests:Vec<Value>=sqlx::query_scalar("SELECT manifest FROM app_packages WHERE tenant=$1 AND active AND manifest->'paymentProvider' IS NOT NULL").bind(&t).fetch_all(&a.db).await?;
    let methods: Vec<String> = manifests
        .iter()
        .flat_map(|m| {
            m["paymentProvider"]["methods"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|method| {
                    method["capabilities"]
                        .as_array()
                        .is_some_and(|v| v.iter().any(|c| c == "vault"))
                })
                .filter_map(|method| {
                    Some(format!(
                        "{}-{}",
                        m["id"].as_str()?.replace('_', "-"),
                        method["id"].as_str()?.replace('_', "-")
                    ))
                })
        })
        .collect();
    Ok(
        json!({"supportedMethods":methods,"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"provider":r.get::<String,_>("provider"),"method":r.get::<String,_>("method"),"environment":r.get::<String,_>("environment"),"metadata":r.get::<Value,_>("metadata"),"createdAt":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub(crate) async fn revoke(a: &App, h: &RequestContext, id: &str, key: &str) -> Result<Value> {
    let (t, customer) = identity(a, h).await?;
    let mut tx = a.db.begin().await?;
    let attempt: Option<String> = sqlx::query_scalar(
        "SELECT attempt_id FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND id=$3",
    )
    .bind(&t)
    .bind(&customer)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let attempt = attempt.ok_or(Error(
        StatusCode::NOT_FOUND,
        "Saved payment method unavailable".into(),
    ))?;
    sqlx::query("SELECT id FROM payment_attempts WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&t)
        .bind(attempt)
        .fetch_one(&mut *tx)
        .await?;

    let row=sqlx::query("SELECT attempt_id,reference,state FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND id=$3 FOR UPDATE").bind(&t).bind(&customer).bind(id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Saved payment method unavailable".into()))?;
    sqlx::query("UPDATE payment_instruments SET state='revoked',revoked_at=coalesce(revoked_at,now()) WHERE tenant=$1 AND id=$2").bind(&t).bind(id).execute(&mut *tx).await?;
    let job = super::enqueue_tx(
        &mut tx,
        &t,
        &row.get::<String, _>("attempt_id"),
        "vault_revoke",
        key,
        &json!({"instrumentId":id,"reference":row.get::<String,_>("reference")}),
    )
    .await?;
    if row.get::<String, _>("state") == "active" {
        sqlx::query(
            "INSERT INTO outbox(tenant,kind,data) VALUES($1,'payment.instrument.revoked',$2)",
        )
        .bind(&t)
        .bind(json!({"instrumentId":id}))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(json!({"id":id,"state":"revoked","jobId":job}))
}
pub(crate) async fn route_list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(list(&a, &h).await?))
}
pub(crate) async fn route_revoke(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        revoke(
            &a,
            &h,
            &id,
            v["requestKey"]
                .as_str()
                .or_else(|| header(&h, "idempotency-key"))
                .ok_or(bad("requestKey required"))?,
        )
        .await?,
    ))
}
