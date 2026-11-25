//! The persisted business approval policy of a shop: its Wasm text source, digest and revision.
use veyndo_core::{Result, State};

/// Stores the policy source of a shop, raising its revision on replacement.
pub async fn save(a: &State, t: &str, wat: &str, digest: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3) ON CONFLICT(tenant) DO UPDATE SET wat=$2,digest=$3,revision=extensions.revision+1",
        t,
        wat,
        digest
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// The digest and revision of the stored policy.
pub struct PolicyState {
    pub digest: String,
    pub revision: i64,
}

/// The digest and revision of a shop's stored policy, if it has one.
pub async fn state(a: &State, t: &str) -> Result<Option<PolicyState>> {
    Ok(sqlx::query_as!(
        PolicyState,
        "SELECT digest,revision FROM extensions WHERE tenant=$1",
        t
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The stored policy source with the digest it was approved under.
pub struct PolicySource {
    pub digest: String,
    pub wat: String,
}

/// The stored policy source of a shop, if it has one.
pub async fn source(a: &State, t: &str) -> Result<Option<PolicySource>> {
    Ok(sqlx::query_as!(
        PolicySource,
        "SELECT digest,wat FROM extensions WHERE tenant=$1",
        t
    )
    .fetch_optional(&a.db)
    .await?)
}
