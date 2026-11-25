//! The shop settings basis and its channel overrides: locked reads, revision-checked writes, the language registration and the dependency probes of a save. Columns checked at compile time.
use crate::domain::defaults::decode;
use crate::domain::patch;
use crate::types::settings::Settings;
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, Transaction};
use veyndo_core::{Error, Result, StatusCode};

/// The shared basis for `default`, otherwise the basis with the channel's overrides applied; `conditions` checks rule conditions in them.
pub async fn scoped_locked(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    channel: &str,
    conditions: &(dyn Fn(&Value) -> Result<()> + Sync),
) -> Result<(Settings, i64)> {
    let row = sqlx::query!(
        "SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR SHARE",
        t
    )
    .fetch_one(&mut **tx)
    .await?;
    let base = decode(row.data)?;
    let revision = row.revision;
    if channel == "default" {
        return Ok((base, revision));
    }
    let exists = sqlx::query_scalar!(
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
    let patch = sqlx::query_scalar!(
        "SELECT data FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2",
        t,
        channel
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok((
        patch::resolve(&base, patch.unwrap_or(json!([])), conditions)?,
        revision,
    ))
}

/// The basis settings of the shop under a share lock on the caller's connection.
pub async fn shared(tx: &mut PgConnection, t: &str) -> Result<Settings> {
    let data = sqlx::query_scalar!(
        "SELECT data FROM commerce_settings WHERE tenant=$1 FOR SHARE",
        t
    )
    .fetch_one(tx)
    .await?;
    decode(data)
}

/// The basis settings row and revision of the shop under an update lock on the caller's connection.
pub async fn for_update(tx: &mut PgConnection, t: &str) -> Result<(Settings, i64)> {
    let row = sqlx::query!(
        "SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_one(tx)
    .await?;
    Ok((decode(row.data)?, row.revision))
}

/// Up to 50 shops after `cursor` whose currency rates refresh automatically, with their settings revision.
pub async fn auto_refresh(a: &veyndo_core::State, cursor: &str) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query!("SELECT tenant,revision FROM commerce_settings WHERE data->'currencies'->>'autoRefresh'='true' AND tenant>$1 ORDER BY tenant LIMIT 50", cursor)
        .fetch_all(&a.db)
        .await?;
    Ok(rows.into_iter().map(|r| (r.tenant, r.revision)).collect())
}

/// The stored basis settings document under an update lock, exactly as saved.
pub async fn basis_for_update(tx: &mut PgConnection, t: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_one(tx)
    .await?)
}

/// The stored basis settings document without a lock.
pub async fn basis(tx: &mut PgConnection, t: &str) -> Result<Value> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM commerce_settings WHERE tenant=$1", t)
            .fetch_one(tx)
            .await?,
    )
}

/// Replaces the basis settings document and advances its revision.
pub async fn replace_basis(tx: &mut PgConnection, t: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE commerce_settings SET data=$1,revision=revision+1 WHERE tenant=$2",
        data,
        t
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Replaces the basis settings document only at the expected revision; the new revision, none when it changed.
pub async fn replace_basis_at(
    tx: &mut PgConnection,
    t: &str,
    data: &Value,
    revision: i64,
) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "UPDATE commerce_settings SET data=$1,revision=revision+1 WHERE tenant=$2 AND revision=$3 RETURNING revision",
        data,
        t,
        revision
    )
    .fetch_optional(tx)
    .await?)
}

/// Replaces the currency settings only at the expected revision; the new revision, none when it changed.
pub async fn replace_currencies_at(
    tx: &mut PgConnection,
    t: &str,
    currencies: &Value,
    revision: i64,
) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "UPDATE commerce_settings SET data=jsonb_set(data,'{currencies}',$1),revision=revision+1 WHERE tenant=$2 AND revision=$3 RETURNING revision",
        currencies,
        t,
        revision
    )
    .fetch_optional(tx)
    .await?)
}

/// Registers an enabled language in the global language table.
pub async fn register_language(tx: &mut PgConnection, locale: &str) -> Result<()> {
    sqlx::query!("SELECT public.veyndo_register_language($1)", locale)
        .execute(tx)
        .await?;
    Ok(())
}

/// Whether a product of the shop is assigned to a tax class outside `tax_ids`.
pub async fn tax_class_stranded(
    tx: &mut PgConnection,
    t: &str,
    tax_ids: &[String],
) -> Result<bool> {
    veyndo_catalog::store::shop_reads::tax_class_stranded(tx, t, tax_ids).await
}

/// The override documents of every channel of the shop.
pub async fn overrides(tx: &mut PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM commerce_overrides WHERE tenant=$1", t)
            .fetch_all(tx)
            .await?,
    )
}

/// The channel and override document of every channel of the shop, by channel, under an update lock.
pub async fn overrides_for_update(tx: &mut PgConnection, t: &str) -> Result<Vec<(String, Value)>> {
    Ok(sqlx::query!(
        "SELECT channel_id,data FROM commerce_overrides WHERE tenant=$1 ORDER BY channel_id FOR UPDATE",
        t
    )
    .fetch_all(tx)
    .await?
    .into_iter()
    .map(|r| (r.channel_id, r.data))
    .collect())
}

/// A channel's override document and its revision.
pub struct Override {
    pub data: Value,
    pub revision: i64,
}

/// The overrides of one channel; none while it inherits the basis unchanged.
pub async fn channel_override(
    tx: &mut PgConnection,
    t: &str,
    channel: &str,
) -> Result<Option<Override>> {
    Ok(sqlx::query_as!(
        Override,
        "SELECT data,revision FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2",
        t,
        channel
    )
    .fetch_optional(tx)
    .await?)
}

/// The revision of one channel's overrides.
pub async fn override_revision(
    tx: &mut PgConnection,
    t: &str,
    channel: &str,
) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT revision FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2",
        t,
        channel
    )
    .fetch_optional(tx)
    .await?)
}

/// Stores a channel's overrides and advances their revision.
pub async fn upsert_override(
    tx: &mut PgConnection,
    t: &str,
    channel: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_overrides(tenant,channel_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,channel_id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_overrides.revision+1",
        t,
        channel,
        data
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// The settings of a channel as one atomic read: revision, cache version and, when `known` is not the current version, the basis and overrides.
pub struct Snapshot {
    pub revision: i64,
    pub version: String,
    pub channel_exists: bool,
    pub data: Option<Value>,
    pub patch: Option<Value>,
}

/// The settings snapshot of a shop's channel; none for an unknown shop.
pub async fn snapshot(
    a: &veyndo_core::State,
    tenant: &str,
    channel: &str,
    known: Option<&str>,
) -> Result<Option<Snapshot>> {
    Ok(sqlx::query_file_as!(
        Snapshot,
        "src/store/settings_snapshot.sql",
        tenant,
        channel,
        known
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The language registry version and, when `known` is not the current version, the registry rows as JSON.
pub struct Registry {
    pub version: String,
    pub data: Option<Value>,
}

/// The global language registry, versioned in the same transaction as every registry change.
pub async fn language_registry(a: &veyndo_core::State, known: Option<&str>) -> Result<Registry> {
    // One read snapshot keeps the version and the rows of the same registry state.
    let mut tx = a.db.begin_read_snapshot().await?;
    let version = veyndo_core::store::read_context::version(&mut tx, "languages").await?;
    let data = if known != Some(version.as_str()) {
        Some(sqlx::query_scalar!(
            r#"SELECT coalesce(jsonb_agg(to_jsonb(l) ORDER BY locale),'[]'::jsonb) AS "data!" FROM (SELECT id,locale,parent_id FROM languages) l"#
        )
        .fetch_one(&mut *tx)
        .await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(Registry { version, data })
}
