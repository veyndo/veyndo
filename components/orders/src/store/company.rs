//! Revisioned tenant company basis and sparse channel overrides, serialized with receipt issuance.
use crate::domain::company_model::{self, CompanyFields, CompanyRecord};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_core::document::view;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, store::history_context,
};
use veyndo_shop::domain::defaults::decode;
use veyndo_shop::store::settings::{basis, shared};
pub async fn lock(tx: &mut Transaction<'_, Postgres>, t: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,731))", t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn records(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    channel: Option<&str>,
) -> Result<Value> {
    let row = sqlx::query!(
        "SELECT data,revision FROM receipt_settings WHERE tenant=$1",
        t
    )
    .fetch_optional(&mut **tx)
    .await?;
    let base = row.as_ref().map(|r| r.data.clone()).unwrap_or(json!({}));
    let rev = row.map(|r| r.revision).unwrap_or(0);
    if let Some(channel) = channel {
        if channel == "default" {
            return Err(bad("Default channel uses the shared company basis"));
        }
        let exists: bool = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2) AS "exists!""#,
            t,
            channel
        )
        .fetch_one(&mut **tx)
        .await?;
        if !exists {
            return Err(Error(
                StatusCode::NOT_FOUND,
                "Sales channel not found".into(),
            ));
        }
        let patch = sqlx::query!(
            "SELECT data,revision FROM company_overrides WHERE tenant=$1 AND channel_id=$2",
            t,
            channel
        )
        .fetch_optional(&mut **tx)
        .await?;
        let data = patch.as_ref().map(|r| r.data.clone()).unwrap_or(json!({}));
        let revision = patch.map(|r| r.revision).unwrap_or(0);
        Ok(
            json!({"data":data,"revision":revision,"baseRevision":rev,"inherited":base,"effective":company_model::resolve(&base,&data),"channelId":channel}),
        )
    } else {
        Ok(json!({"data":base,"revision":rev}))
    }
}
/// The basis, or one channel's override with what it inherits, read under the company lock.
pub async fn read(a: &State, h: &RequestContext, t: &str, channel: Option<&str>) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    lock(&mut tx, t).await?;
    let value = records(&mut tx, t, channel).await?;
    tx.commit().await?;
    Ok(value)
}
/// Revision-checked save of the basis or of one channel's sparse override; `t` is the admitted merchant shop.
pub async fn save(
    a: &State,
    h: &RequestContext,
    t: &str,
    v: Value,
    channel: Option<&str>,
) -> Result<Value> {
    let request: CompanyRecord = view(&v);
    let expected = request
        .revision
        .as_i64()
        .filter(|n| *n >= 0)
        .ok_or(bad("revision required"))?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    lock(&mut tx, t).await?;
    let settings = shared(&mut tx, t).await?;
    company_model::validate(&request.data, &settings, channel.is_some())?;
    let current: CompanyRecord = view(&records(&mut tx, t, channel).await?);
    if current.revision.as_i64() != Some(expected)
        || channel.is_some() && current.base_revision != request.base_revision
    {
        return Err(conflict("Company settings changed; reload before saving"));
    }
    let data = if channel.is_some() {
        request.data.clone()
    } else {
        company_model::resolve(&request.data, &json!({}))
    };
    let effective = if channel.is_some() {
        company_model::resolve(&current.inherited, &data)
    } else {
        data.clone()
    };
    company_model::validate(&effective, &settings, false)?;
    // A basis edit also checks all dependants: inherited country changes cannot leave a foreign subdivision.
    if channel.is_none() {
        let patches: Vec<Value> =
            sqlx::query_scalar!("SELECT data FROM company_overrides WHERE tenant=$1", t)
                .fetch_all(&mut *tx)
                .await?;
        for patch in patches {
            company_model::validate(&company_model::resolve(&data, &patch), &settings, false)?;
        }
    }
    let effective_logo = view::<CompanyFields>(&effective).logo_id;
    if let Some(logo) = effective_logo.as_str().filter(|s| !s.is_empty()) {
        let owned: bool = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM company_logos WHERE tenant=$1 AND id=$2) AS "exists!""#,
            t,
            logo
        )
        .fetch_one(&mut *tx)
        .await?;
        if !owned {
            return Err(bad("Company logo unavailable in this shop"));
        }
    }
    if let Some(channel) = channel {
        sqlx::query!("INSERT INTO company_overrides(tenant,channel_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,channel_id) DO UPDATE SET data=EXCLUDED.data,revision=company_overrides.revision+1", t, channel, data).execute(&mut *tx).await?;
    } else {
        sqlx::query!("INSERT INTO receipt_settings(tenant,data) VALUES($1,$2) ON CONFLICT(tenant) DO UPDATE SET data=EXCLUDED.data,revision=receipt_settings.revision+1", t, data).execute(&mut *tx).await?;
    }
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        t,
        "settings.master_data_changed",
        &json!({"revision":expected+1,"salesChannelId":channel}),
    )
    .await?;
    let value = records(&mut tx, t, channel).await?;
    tx.commit().await?;
    Ok(value)
}
pub async fn seller(tx: &mut Transaction<'_, Postgres>, t: &str, channel: &str) -> Result<Value> {
    lock(tx, t).await?;
    let basis: CompanyRecord = view(&records(tx, t, None).await?);
    if basis.revision == 0 {
        return Err(bad("Configure seller details before creating receipts"));
    }
    let patch: Option<Value> = sqlx::query_scalar!(
        "SELECT data FROM company_overrides WHERE tenant=$1 AND channel_id=$2",
        t,
        channel
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(company_model::resolve(
        &basis.data,
        &patch.unwrap_or(json!({})),
    ))
}

/// Atomic release validates the resulting basis, all channel dependants and tenant-owned logo references.
pub async fn validate_release(tx: &mut Transaction<'_, Postgres>, t: &str) -> Result<()> {
    let raw = basis(tx, t).await?;
    let settings = decode(raw)?;
    let base: Option<Value> =
        sqlx::query_scalar!("SELECT data FROM receipt_settings WHERE tenant=$1", t)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(base) = base {
        let mut values = vec![base.clone()];
        let patches: Vec<Value> =
            sqlx::query_scalar!("SELECT data FROM company_overrides WHERE tenant=$1", t)
                .fetch_all(&mut **tx)
                .await?;
        values.extend(
            patches
                .into_iter()
                .map(|p| company_model::resolve(&base, &p)),
        );
        for value in values {
            company_model::validate(&value, &settings, false)?;
            let logo = view::<CompanyFields>(&value).logo_id;
            if let Some(id) = logo.as_str().filter(|s| !s.is_empty()) {
                let exists: bool = sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM company_logos WHERE tenant=$1 AND id=$2) AS "exists!""#, t, id)
                .fetch_one(&mut **tx)
                .await?;
                if !exists {
                    return Err(bad("Release company logo unavailable"));
                }
            }
        }
    }
    Ok(())
}

/// The effective company data of a channel and, while no brand or legal name is configured, the shop's chosen name.
pub async fn public_source(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    channel: &str,
) -> Result<(Value, Option<String>)> {
    lock(tx, t).await?;
    let base: CompanyRecord = view(&records(tx, t, None).await?);
    let patch: Option<Value> = sqlx::query_scalar!(
        "SELECT data FROM company_overrides WHERE tenant=$1 AND channel_id=$2",
        t,
        channel
    )
    .fetch_optional(&mut **tx)
    .await?;
    let data = company_model::resolve(&base.data, &patch.unwrap_or(json!({})));
    // Unconfigured shop presentation uses its chosen name, never a fabricated legal issuer.
    let default_brand: Option<String> = if data.get("brandName").is_none_or(Value::is_null)
        && view::<CompanyFields>(&data)
            .name
            .as_str()
            .is_none_or(|name| name.trim().is_empty())
    {
        veyndo_core::store::tenants::name(&mut **tx, t).await?
    } else {
        None
    };
    Ok((data, default_brand))
}
