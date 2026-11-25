//! The legal acknowledgement and privacy consent receipt a purchase freezes into its order, share-locked on the purchase connection. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// The cart's legal acknowledgement document, share-locked; `None` when the buyer acknowledged nothing.
pub async fn share_acceptance(
    conn: &mut PgConnection,
    tenant: &str,
    cart: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM legal_acceptances WHERE tenant=$1 AND cart_id=$2 FOR SHARE",
        tenant,
        cart
    )
    .fetch_optional(conn)
    .await?)
}

/// The root consent receipt a cart's consent resolves to, share-locked.
pub struct ConsentReceipt {
    pub cart_id: String,
    pub policy_version: String,
    pub data: Value,
    /// The receipt has not expired.
    pub fresh: bool,
}

/// The cart's consent receipt in the channel (through its one-hop alias), share-locked.
/// Aliases are server-created, one hop and immutable; both sides are tenant/channel scoped.
pub async fn consent_receipt(
    conn: &mut PgConnection,
    tenant: &str,
    cart: &str,
    channel: &str,
) -> Result<Option<ConsentReceipt>> {
    Ok(sqlx::query_as!(
        ConsentReceipt,
        r#"SELECT root.cart_id,root.policy_version,root.data,root.expires_at>now() AS "fresh!" FROM privacy_consents own JOIN privacy_consents root ON root.tenant=own.tenant AND root.channel_id=own.channel_id AND root.cart_id=coalesce(own.source_cart_id,own.cart_id) AND root.source_cart_id IS NULL WHERE own.tenant=$1 AND own.cart_id=$2 AND own.channel_id=$3 FOR SHARE OF root"#,
        tenant,
        cart,
        channel
    )
    .fetch_optional(conn)
    .await?)
}
