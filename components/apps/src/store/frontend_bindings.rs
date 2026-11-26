//! The global hosted-frontend address registry through its owners: collisions with other shops (core), the bound channel (marketing) and the shop's revisioned bindings (shop).
use sqlx::PgConnection;
use veyndo_core::Result;
pub use veyndo_shop::store::app_frontends::{
    Binding, NewBinding, experience_app, lock, remove, save,
};

/// Whether the address is the ID of another shop.
pub async fn names_other_shop(conn: &mut PgConnection, alias: &str, t: &str) -> Result<bool> {
    Ok(alias != t && veyndo_core::store::tenants::exists(conn, alias).await?)
}

/// Whether the shop has the sales channel.
pub async fn channel_exists(conn: &mut PgConnection, t: &str, channel: &str) -> Result<bool> {
    veyndo_marketing::store::app_channels::exists(conn, t, channel).await
}
