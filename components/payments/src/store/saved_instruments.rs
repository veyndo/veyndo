//! Customer-owned saved instruments expose only redacted metadata; immediate revocation and provider cleanup share payment jobs.
use crate::store::jobs::enqueue_tx;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, State, StatusCode};

/// The provider methods of an installed manifest.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct VaultManifest {
    id: Value,
    #[serde(deserialize_with = "lenient")]
    payment_provider: VaultProvider,
}

/// The methods of a payment provider contract.
#[derive(Deserialize, Default)]
#[serde(default)]
struct VaultProvider {
    methods: Value,
}

/// A provider method and its capabilities.
#[derive(Deserialize, Default)]
#[serde(default)]
struct VaultMethod {
    id: Value,
    capabilities: Value,
}

pub async fn list(a: &State, t: &str, customer: &str) -> Result<Value> {
    let rows=sqlx::query!("SELECT id,provider,method,environment,metadata,created_at::text FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND state='active' ORDER BY created_at DESC LIMIT 32", t, customer).fetch_all(&a.db).await?;
    let manifests:Vec<Value>=sqlx::query_scalar!("SELECT manifest FROM app_packages WHERE tenant=$1 AND active AND manifest->'paymentProvider' IS NOT NULL", t).fetch_all(&a.db).await?;
    let methods: Vec<String> = manifests
        .iter()
        .map(view::<VaultManifest>)
        .flat_map(|m| {
            m.payment_provider
                .methods
                .as_array()
                .into_iter()
                .flatten()
                .map(view::<VaultMethod>)
                .filter(|method| {
                    method
                        .capabilities
                        .as_array()
                        .is_some_and(|v| v.iter().any(|c| c == "vault"))
                })
                .filter_map(|method| {
                    Some(format!(
                        "{}-{}",
                        m.id.as_str()?.replace('_', "-"),
                        method.id.as_str()?.replace('_', "-")
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Ok(
        json!({"supportedMethods":methods,"elements":rows.iter().map(|r|json!({"id":r.id,"provider":r.provider,"method":r.method,"environment":r.environment,"metadata":r.metadata,"createdAt":r.created_at})).collect::<Vec<_>>() }),
    )
}
pub async fn revoke(a: &State, t: &str, customer: &str, id: &str, key: &str) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    let attempt: Option<String> = sqlx::query_scalar!(
        "SELECT attempt_id FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND id=$3",
        t,
        customer,
        id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let attempt = attempt.ok_or(Error(
        StatusCode::NOT_FOUND,
        "Saved payment method unavailable".into(),
    ))?;
    sqlx::query!(
        "SELECT id FROM payment_attempts WHERE tenant=$1 AND id=$2 FOR UPDATE",
        t,
        attempt
    )
    .fetch_one(&mut *tx)
    .await?;

    let row=sqlx::query!("SELECT attempt_id,reference,state FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND id=$3 FOR UPDATE", t, customer, id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Saved payment method unavailable".into()))?;
    sqlx::query!("UPDATE payment_instruments SET state='revoked',revoked_at=coalesce(revoked_at,now()) WHERE tenant=$1 AND id=$2", t, id).execute(&mut *tx).await?;
    let job = enqueue_tx(
        &mut tx,
        t,
        &row.attempt_id,
        "vault_revoke",
        key,
        &json!({"instrumentId":id,"reference":row.reference}),
    )
    .await?;
    if row.state == "active" {
        veyndo_core::store::outbox::publish_on(
            &mut *tx,
            t,
            "payment.instrument.revoked",
            &json!({"instrumentId":id}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(json!({"id":id,"state":"revoked","jobId":job}))
}
