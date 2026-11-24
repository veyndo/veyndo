//! Selective company identity release copies only linked immutable logo bytes and validates the final company aggregate.
use crate::store::order_mutation::Tx;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad, conflict};

pub async fn snapshot(
    tx: &mut Tx<'_>,
    t: &str,
    result: &mut serde_json::Map<String, Value>,
) -> Result<()> {
    if let Some(data) = sqlx::query_scalar::<_, Value>(
        "SELECT data FROM receipt_settings WHERE tenant=$1 FOR UPDATE",
    )
    .bind(t)
    .fetch_optional(&mut **tx)
    .await?
    {
        result.insert("company".into(), profile(tx, t, data).await?);
    }
    for row in sqlx::query("SELECT channel_id,data FROM company_overrides WHERE tenant=$1 ORDER BY channel_id FOR UPDATE").bind(t).fetch_all(&mut **tx).await?{
  result.insert(format!("company-channel:{}",row.get::<String,_>("channel_id")),profile(tx,t,row.get("data")).await?);
 }
    Ok(())
}
async fn profile(tx: &mut Tx<'_>, t: &str, data: Value) -> Result<Value> {
    let mut result = json!({"data":data});
    if let Some(id) = result["data"]["logoId"].as_str().filter(|s| !s.is_empty()) {
        let digest: String =
            sqlx::query_scalar("SELECT digest FROM company_logos WHERE tenant=$1 AND id=$2")
                .bind(t)
                .bind(id)
                .fetch_one(&mut **tx)
                .await?;
        result["logoDigest"] = json!(digest);
    }
    Ok(result)
}
/// The JSON fields `publish` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PublishInput {
    data: Value,
    #[serde(deserialize_with = "lenient")]
    logo_digest: Option<String>,
}

pub async fn publish(
    tx: &mut Tx<'_>,
    live: &str,
    stage: &str,
    key: &str,
    value: &Value,
) -> Result<()> {
    let input: PublishInput = view(value);
    let mut data = input.data.clone();
    if let Some(id) = data["logoId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM company_logos WHERE tenant=$1 AND digest=$2")
                .bind(live)
                .bind(input.logo_digest.as_deref())
                .fetch_optional(&mut **tx)
                .await?;
        if let Some(existing) = existing {
            data["logoId"] = json!(existing);
        } else {
            let n=sqlx::query("INSERT INTO company_logos(tenant,id,content,digest,mime,width,height) SELECT $1,id,content,digest,mime,width,height FROM company_logos WHERE tenant=$2 AND id=$3 AND digest=$4 ON CONFLICT(tenant,id) DO UPDATE SET digest=company_logos.digest WHERE company_logos.digest=EXCLUDED.digest").bind(live).bind(stage).bind(id).bind(input.logo_digest.as_deref()).execute(&mut **tx).await?.rows_affected();
            if n != 1 {
                return Err(conflict("Company logo changed or unavailable"));
            }
        }
    }
    if key == "company" {
        sqlx::query("INSERT INTO receipt_settings(tenant,data) VALUES($1,$2) ON CONFLICT(tenant) DO UPDATE SET data=EXCLUDED.data,revision=receipt_settings.revision+1").bind(live).bind(&data).execute(&mut **tx).await?;
    } else {
        let channel = key
            .strip_prefix("company-channel:")
            .ok_or(bad("Invalid company release scope"))?;
        sqlx::query("INSERT INTO company_overrides(tenant,channel_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,channel_id) DO UPDATE SET data=EXCLUDED.data,revision=company_overrides.revision+1").bind(live).bind(channel).bind(&data).execute(&mut **tx).await?;
    }
    veyndo_core::outbox::publish_on(
        &mut **tx,
        live,
        "settings.master_data_changed",
        &json!({"release":key}),
    )
    .await?;
    Ok(())
}

/// Return the canonical published unit after digest deduplication; baseline must match actual live IDs.
pub async fn live_value(tx: &mut Tx<'_>, t: &str, key: &str) -> Result<Value> {
    let data: Value = if key == "company" {
        sqlx::query_scalar("SELECT data FROM receipt_settings WHERE tenant=$1")
            .bind(t)
            .fetch_one(&mut **tx)
            .await?
    } else {
        sqlx::query_scalar("SELECT data FROM company_overrides WHERE tenant=$1 AND channel_id=$2")
            .bind(t)
            .bind(&key[16..])
            .fetch_one(&mut **tx)
            .await?
    };
    profile(tx, t, data).await
}
