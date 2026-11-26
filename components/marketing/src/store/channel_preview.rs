//! One-use channel preview tickets and the grants they rotate into, bound to a live merchant session and an exact channel revision. Ticket columns are checked at compile time; sessions and memberships come from the identity store, hosted frontend bindings from the shop store.
use serde_json::Value;
use veyndo_core::{Result, State};
use veyndo_identity::store::preview_sessions;
use veyndo_shop::store::channel_settings;

/// Whether `digest` is a live session of `user`; the caller runs it in the system scope.
pub async fn session_alive(a: &State, digest: &str, user: Option<&str>) -> Result<bool> {
    preview_sessions::alive(a, digest, user).await
}

/// The channel's current revision, if it exists.
pub async fn channel_revision(a: &State, tenant: &str, id: &str) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT revision FROM sales_channels WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Whether the hosted frontend `alias` serves this channel.
pub async fn frontend_of_channel(a: &State, tenant: &str, id: &str, alias: &str) -> Result<bool> {
    channel_settings::frontend_serves(a, tenant, id, alias).await
}

/// A new sixty-second ticket; expired tickets of the shop are removed first.
pub struct Ticket<'a> {
    pub digest: String,
    pub tenant: &'a str,
    pub channel: &'a str,
    pub session_digest: &'a str,
    pub revision: i64,
    pub alias: Option<&'a str>,
}

/// Stores a ticket after removing the shop's expired ones.
pub async fn issue(a: &State, ticket: Ticket<'_>) -> Result<()> {
    sqlx::query!(
        "DELETE FROM channel_previews WHERE expires_at<now() AND tenant=$1",
        ticket.tenant
    )
    .execute(&a.db)
    .await?;
    sqlx::query!("INSERT INTO channel_previews(digest,tenant,channel,session_digest,channel_revision,alias,expires_at) VALUES($1,$2,$3,$4,$5,$6,now()+interval '60 seconds')",
        ticket.digest, ticket.tenant, ticket.channel, ticket.session_digest, ticket.revision, ticket.alias)
    .execute(&a.db)
    .await?;
    Ok(())
}

/// The ticket that was redeemed: its frontend alias and channel.
pub struct Redeemed {
    pub alias: Option<String>,
    pub channel: String,
}

/// Atomically rotates an unredeemed, unexpired ticket into a fifteen-minute grant.
pub async fn redeem(
    a: &State,
    grant_digest: &str,
    ticket_digest: &str,
    (tenant, channel): (&str, &str),
    host: Option<&str>,
) -> Result<Option<Redeemed>> {
    Ok(sqlx::query_as!(Redeemed, "UPDATE channel_previews SET digest=$1,redeemed=true,expires_at=now()+interval '15 minutes' WHERE digest=$2 AND tenant=$3 AND channel=$4 AND NOT redeemed AND expires_at>now() AND alias IS NOT DISTINCT FROM $5::text RETURNING alias,channel",
        grant_digest, ticket_digest, tenant, channel, host)
    .fetch_optional(&a.db)
    .await?)
}

/// The membership behind a valid grant.
pub struct Grant {
    pub alias: Option<String>,
    pub role: String,
    pub permissions: Value,
}

/// The grant of a live session of an active member for this exact channel revision; the caller runs it in the system scope.
pub async fn grant(
    a: &State,
    grant_digest: &str,
    (tenant, channel, revision): (&str, &str, i64),
) -> Result<Option<Grant>> {
    let Some(ticket) = sqlx::query!("SELECT alias,session_digest FROM channel_previews WHERE digest=$1 AND tenant=$2 AND channel=$3 AND channel_revision=$4 AND redeemed AND expires_at>now()",
        grant_digest, tenant, channel, revision)
    .fetch_optional(&a.db)
    .await?
    else {
        return Ok(None);
    };
    Ok(preview_sessions::member(a, &ticket.session_digest, tenant)
        .await?
        .map(|m| Grant {
            alias: ticket.alias,
            role: m.role,
            permissions: m.permissions,
        }))
}
