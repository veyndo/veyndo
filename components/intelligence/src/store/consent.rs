//! Consent receipts: the shared root receipt of a cart, its aliases, the change log and withdrawal erasure. Columns checked at compile time.
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// The root receipt a cart's consent resolves to.
pub struct Receipt {
    /// The cart that owns the root receipt.
    pub cart_id: String,
    pub policy_version: String,
    pub data: Value,
    /// Whether the receipt has not expired.
    pub fresh: bool,
}

/// The fields of a stored receipt document that decide processing.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ReceiptView {
    /// Purpose → choice; only a stored `true` grants a purpose.
    pub choices: Value,
}

impl ReceiptView {
    /// Whether the receipt grants `purpose`.
    pub fn grants(&self, purpose: &str) -> bool {
        self.choices.get(purpose) == Some(&Value::Bool(true))
    }
}

/// The root receipt of a cart in a channel, share-locked; aliases are server-created, one hop and immutable.
pub async fn receipt(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    channel: &str,
) -> Result<Option<Receipt>> {
    Ok(sqlx::query_as!(
        Receipt,
        r#"SELECT root.cart_id,root.policy_version,root.data,root.expires_at>now() AS "fresh!" FROM privacy_consents own JOIN privacy_consents root ON root.tenant=own.tenant AND root.channel_id=own.channel_id AND root.cart_id=coalesce(own.source_cart_id,own.cart_id) AND root.source_cart_id IS NULL WHERE own.tenant=$1 AND own.cart_id=$2 AND own.channel_id=$3 FOR SHARE OF root"#,
        tenant,
        cart,
        channel
    )
    .fetch_optional(e)
    .await?)
}

/// Locks the cart row so consent changes of one cart serialize.
pub async fn lock_cart(e: impl PgExecutor<'_>, tenant: &str, cart: &str) -> Result<()> {
    veyndo_checkout::store::cart_signals::lock_cart(e, tenant, cart).await
}

/// Consent changes of a receipt during the last day.
pub async fn changes_today(e: impl PgExecutor<'_>, tenant: &str, owner: &str) -> Result<i64> {
    veyndo_shop::store::consent_log::changes_today(e, tenant, owner).await
}

/// The unexpired receipt document of the owner under `policy`, row-locked.
pub async fn current_locked(
    e: impl PgExecutor<'_>,
    tenant: &str,
    owner: &str,
    channel: &str,
    policy: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM privacy_consents WHERE tenant=$1 AND cart_id=$2 AND channel_id=$3 AND policy_version=$4 AND expires_at>now() FOR UPDATE",
        tenant,
        owner,
        channel,
        policy
    )
    .fetch_optional(e)
    .await?)
}

/// Saves the owner's receipt, valid for `days` days.
pub async fn save(
    e: impl PgExecutor<'_>,
    tenant: &str,
    owner: &str,
    channel: &str,
    policy: &str,
    receipt: &Value,
    days: i32,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO privacy_consents(tenant,cart_id,channel_id,policy_version,data,expires_at) VALUES($1,$2,$3,$4,$5,now()+make_interval(days=>$6)) ON CONFLICT(tenant,cart_id,channel_id) DO UPDATE SET policy_version=EXCLUDED.policy_version,data=EXCLUDED.data,expires_at=EXCLUDED.expires_at",
        tenant,
        owner,
        channel,
        policy,
        receipt,
        days
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Appends an entry to the consent log.
pub async fn log(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    channel: &str,
    entry: &Value,
) -> Result<()> {
    veyndo_shop::store::consent_log::log(e, tenant, cart, channel, entry).await
}

/// Deletes the experiment exposure of a session.
pub async fn forget_exposure(e: impl PgExecutor<'_>, tenant: &str, session: &str) -> Result<()> {
    veyndo_checkout::store::cart_signals::forget_exposure(e, tenant, session).await
}

/// Deletes the personalization signals of the sessions.
pub async fn forget_personalization(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    sessions: &[String],
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM session_signals WHERE tenant=$1 AND session=ANY($2)",
        tenant,
        sessions
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM session_signal_events WHERE tenant=$1 AND session=ANY($2) AND purpose='personalization'",
        tenant,
        sessions
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Deletes the analytics events of the sessions.
pub async fn forget_analytics(
    e: impl PgExecutor<'_>,
    tenant: &str,
    sessions: &[String],
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM session_signal_events WHERE tenant=$1 AND session=ANY($2) AND purpose='analytics'",
        tenant,
        sessions
    )
    .execute(e)
    .await?;
    Ok(())
}

/// The cart whose receipt this cart continues, if the cart has a receipt row.
pub async fn owner(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    channel: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT coalesce(source_cart_id,cart_id) AS "owner!" FROM privacy_consents WHERE tenant=$1 AND cart_id=$2 AND channel_id=$3"#,
        tenant,
        cart,
        channel
    )
    .fetch_optional(e)
    .await?)
}

/// The carts that share the owner's receipt.
pub async fn sharing_carts(
    e: impl PgExecutor<'_>,
    tenant: &str,
    channel: &str,
    owner: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT cart_id FROM privacy_consents WHERE tenant=$1 AND channel_id=$2 AND (cart_id=$3 OR source_cart_id=$3) ORDER BY cart_id",
        tenant,
        channel,
        owner
    )
    .fetch_all(e)
    .await?)
}

/// Points a successor cart at the source receipt; false when the cart already has one.
pub async fn alias(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    channel: &str,
    source: &str,
) -> Result<bool> {
    Ok(sqlx::query!(
        r#"INSERT INTO privacy_consents(tenant,cart_id,channel_id,policy_version,data,expires_at,source_cart_id) VALUES($1,$2,$3,'','{"choices":{},"decided":false}',now(),$4) ON CONFLICT(tenant,cart_id,channel_id) DO NOTHING"#,
        tenant,
        cart,
        channel,
        source
    )
    .execute(e)
    .await?
    .rows_affected()
        == 1)
}
