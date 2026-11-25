//! Saved instruments a verified receipt may add: only for the original account's consent, only redacted metadata.
use crate::domain::attempt::Attempt;
use crate::domain::receipt_ref::reference;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad, conflict};

/// Optional instrument storage can never roll back an already verified financial receipt.
pub async fn observe(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    v: &Value,
) -> Result<()> {
    if v.get("vault").is_none() {
        return Ok(());
    }
    sqlx::query!("SAVEPOINT vault_metadata")
        .execute(&mut **tx)
        .await?;
    let status = if accept(tx, p, v).await.is_err() {
        sqlx::query!("ROLLBACK TO SAVEPOINT vault_metadata")
            .execute(&mut **tx)
            .await?;
        "failed"
    } else {
        "saved"
    };
    sqlx::query!("RELEASE SAVEPOINT vault_metadata")
        .execute(&mut **tx)
        .await?;
    sqlx::query!("UPDATE payment_attempts SET provider_context=jsonb_set(provider_context,'{vaultStatus}',$3) WHERE tenant=$1 AND id=$2", p.tenant, p.id, json!(status)).execute(&mut **tx).await?;
    Ok(())
}
/// The JSON fields `accept` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AcceptInput {
    #[serde(deserialize_with = "lenient")]
    status: Option<String>,
}

/// A financial provider receipt may add a handle only for the original account's explicit save consent.
pub async fn accept(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    v: &Value,
) -> Result<()> {
    let input: AcceptInput = view(v);
    let Some(vault) = v.get("vault") else {
        return Ok(());
    };
    let f = &p.context["funding"];
    if f["save"] != true
        || f["customerId"] != vault["customerId"]
        || f["consentId"] != vault["consentId"]
        || !["captured", "authorized", "partially_captured"]
            .contains(&input.status.as_deref().unwrap_or(""))
    {
        return Err(bad(
            "Saved instrument lacks original customer consent and successful payment",
        ));
    }
    let reference = reference(&vault["reference"])?;
    let metadata = projection(vault)?;
    let id = f["consentId"]
        .as_str()
        .ok_or(bad("Consent identity required"))?;
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,515))",
        format!("{}:{}", p.tenant, f["customerId"])
    )
    .execute(&mut **tx)
    .await?;
    let count:i64=sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM payment_instruments WHERE tenant=$1 AND customer_id=$2 AND state='active' AND id<>$3"#, p.tenant, f["customerId"].as_str().unwrap_or(""), id).fetch_one(&mut **tx).await?;
    if count >= 32 {
        return Err(bad("Maximum 32 saved payment methods per customer"));
    }
    sqlx::query!("INSERT INTO payment_instruments(tenant,id,customer_id,provider,adapter_version,account_ref,environment,method,reference,attempt_id,metadata,consent_version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(tenant,id) DO NOTHING", p.tenant, id, f["customerId"].as_str().unwrap_or(""), p.provider, p.adapter_version, p.context["accountRef"].as_str().unwrap_or(""), p.environment, p.context["method"].as_str().unwrap_or(""), reference, p.id, metadata, f["consentVersion"].as_str().unwrap_or("")).execute(&mut **tx).await?;
    let original: String = sqlx::query_scalar!(
        "SELECT reference FROM payment_instruments WHERE tenant=$1 AND id=$2",
        p.tenant,
        id
    )
    .fetch_one(&mut **tx)
    .await?;
    if original != reference {
        return Err(conflict("Saved payment handle changed"));
    }
    Ok(())
}
fn projection(v: &Value) -> Result<Value> {
    let mut result = json!({});
    for key in ["brand", "last4"] {
        if let Some(s) = v[key].as_str() {
            if s.is_empty()
                || s.len() > 32
                || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b' ')
                || (key == "last4" && (s.len() != 4 || !s.bytes().all(|c| c.is_ascii_digit())))
            {
                return Err(bad("Invalid redacted instrument metadata"));
            }
            result[key] = json!(s);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::projection;
    use serde_json::json;
    #[test]
    fn public_instrument_metadata_never_contains_provider_tokens_or_customer_data() {
        assert_eq!(projection(&json!({"brand":"Visa","last4":"4242","token":"secret","email":"private@example.test","customerId":"private"})).unwrap(),json!({"brand":"Visa","last4":"4242"}));
        assert!(projection(&json!({"brand":"line\nbreak"})).is_err());
    }
}
