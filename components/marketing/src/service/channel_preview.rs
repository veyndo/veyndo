//! Merchant preview grants of paused or private channels: bound to the current session, membership, channel revision and host.
use serde_json::Value;
use sqlx::Row;
use veyndo_core::request_context::HeaderReader;
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_identity::permit;
use veyndo_runtime::tenant_scope::{Scope, scoped};

/// The preview grant a browser presents in its host-only cookie.
pub fn cookie(h: &impl HeaderReader) -> Option<String> {
    header(h, "cookie")?
        .split(';')
        .map(str::trim)
        .find_map(|pair| pair.strip_prefix("veyndo_preview=").map(str::to_owned))
}
/// A preview ticket or grant: 64 hexadecimal characters.
pub fn token(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit())
}
/// The grant still belongs to a live session of an active member allowed to read channels, for this exact channel revision and host.
pub async fn valid(
    a: &State,
    (t, channel, revision): (&str, &str, i64),
    grant: &str,
    host: Option<&str>,
) -> Result<bool> {
    if !token(grant) {
        return Ok(false);
    }
    let row = scoped(Scope::System,sqlx::query("SELECT p.alias,m.role,m.permissions FROM channel_previews p JOIN user_sessions s ON s.digest=p.session_digest JOIN memberships m ON m.user_id=s.user_id AND m.tenant=p.tenant WHERE p.digest=$1 AND p.tenant=$2 AND p.channel=$3 AND p.channel_revision=$4 AND p.redeemed AND p.expires_at>now() AND s.expires_at>now() AND m.active")
        .bind(hash(grant)).bind(t).bind(channel).bind(revision).fetch_optional(&a.db)).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let alias: Option<String> = row.get("alias");
    // Server-to-server Store API has no host mount but still proves the exact tenant/channel/revision.
    if host.is_some_and(|h| alias.as_deref() != Some(h)) {
        return Ok(false);
    }
    let mut identity = RequestContext::new();
    identity.insert(
        "x-rac-role",
        row.get::<String, _>("role")
            .parse()
            .map_err(|_| bad("Invalid membership"))?,
    );
    let permissions: Value = row.get("permissions");
    if !permissions.is_null() {
        identity.insert(
            "x-rac-permissions",
            permissions
                .to_string()
                .parse()
                .map_err(|_| bad("Invalid permissions"))?,
        );
    }
    Ok(permit(&identity, "channels.read").is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grants_are_hexadecimal_and_read_from_the_preview_cookie() {
        assert!(token(&"a".repeat(64)));
        assert!(!token(&"g".repeat(64)));
        assert!(!token("abc"));
        let mut h = RequestContext::new();
        h.insert("cookie", "x=1; veyndo_preview=abc".parse().unwrap());
        assert_eq!(cookie(&h).as_deref(), Some("abc"));
    }
}
