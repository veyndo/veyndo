//! The storefront origin that may frame the hosted checkout: exactly the mounted shop and sales channel, over plain HTTPS.
use veyndo_core::host_names::configured_alias;
use veyndo_core::{Error, Result, StatusCode, bad, validate_tenant};

/// Admits the parent origin when it is a hosted frontend of this shop and channel.
pub async fn registered_parent(
    conn: &mut sqlx::PgConnection,
    t: &str,
    channel: &str,
    origin: &str,
) -> Result<()> {
    let url = reqwest::Url::parse(origin).map_err(|_| bad("Invalid checkout parent"))?;
    let suffix = std::env::var("SHOP_DOMAIN_SUFFIX").unwrap_or_else(|_| "veyndo.ai".into());
    let host = url.host_str().ok_or(bad("Invalid checkout parent"))?;
    let legacy = std::env::var("LEGACY_DOMAINS").unwrap_or_default();
    let alias =
        configured_alias(host, &suffix, &legacy).ok_or(bad("Checkout parent unavailable"))?;
    validate_tenant(alias)?;
    if url.scheme() != "https"
        || url.port().is_some()
        || url.origin().ascii_serialization() != *origin
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(bad("Invalid checkout parent"));
    }
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND channel=$2 AND alias=$3)",
    )
    .bind(t)
    .bind(channel)
    .bind(alias)
    .fetch_one(conn)
    .await?;
    if !allowed {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Checkout parent is not connected to this shop".into(),
        ));
    }
    Ok(())
}
