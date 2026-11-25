//! The provider accounts of a shop's channels and the installed payment packages they belong to. Columns checked at compile time.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;

/// The package version is still installed and active; share-locked so an uninstall waits for the binding.
pub async fn share_package_active(
    tx: &mut PgConnection,
    tenant: &str,
    app: &str,
    version: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT active FROM app_packages WHERE tenant=$1 AND id=$2 AND version=$3 FOR SHARE",
        tenant,
        app,
        version
    )
    .fetch_optional(tx)
    .await?
    .unwrap_or(false))
}

/// The account of one app in one channel.
pub struct AccountKey<'a> {
    pub tenant: &'a str,
    pub app: &'a str,
    pub channel: &'a str,
    pub account_ref: &'a str,
    pub environment: &'a str,
}

/// Binds (or replaces) the channel's account of the app.
pub async fn bind(
    tx: &mut PgConnection,
    k: &AccountKey<'_>,
    ready: bool,
    data: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO payment_provider_accounts(tenant,app,channel,account_ref,environment,ready,data) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(tenant,app,channel) DO UPDATE SET account_ref=EXCLUDED.account_ref,environment=EXCLUDED.environment,ready=EXCLUDED.ready,data=EXCLUDED.data,updated_at=now()", k.tenant, k.app, k.channel, k.account_ref, k.environment, ready, data).execute(tx).await?;
    Ok(())
}

/// Updates the readiness of the channel's account only when it is still that account in that environment.
pub async fn refresh(
    tx: &mut PgConnection,
    k: &AccountKey<'_>,
    ready: bool,
    data: &Value,
) -> Result<()> {
    sqlx::query!("UPDATE payment_provider_accounts SET ready=$6,data=$7,updated_at=now() WHERE tenant=$1 AND app=$2 AND channel=$3 AND account_ref=$4 AND environment=$5", k.tenant, k.app, k.channel, k.account_ref, k.environment, ready, data).execute(tx).await?;
    Ok(())
}

/// An installed package with a payment provider contract.
pub struct ProviderPackage {
    pub id: String,
    pub version: String,
    pub active: bool,
    pub contract: Option<Value>,
}

/// The shop's packages that declare a payment provider, by id.
pub async fn packages<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Vec<ProviderPackage>> {
    Ok(sqlx::query_as!(
        ProviderPackage,
        "SELECT id,version,active,manifest->'paymentProvider' AS contract FROM app_packages WHERE tenant=$1 AND manifest ? 'paymentProvider' ORDER BY id",
        tenant
    )
    .fetch_all(db)
    .await?)
}

/// A channel account as the merchant listing shows it.
pub struct Account {
    pub app: String,
    pub channel: String,
    pub account_ref: String,
    pub environment: String,
    pub ready: bool,
    pub updated_at: String,
}

/// The shop's provider accounts by app and channel.
pub async fn accounts<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Vec<Account>> {
    Ok(sqlx::query_as!(
        Account,
        r#"SELECT app,channel,account_ref,environment,ready,updated_at::text AS "updated_at!" FROM payment_provider_accounts WHERE tenant=$1 ORDER BY app,channel"#,
        tenant
    )
    .fetch_all(db)
    .await?)
}
