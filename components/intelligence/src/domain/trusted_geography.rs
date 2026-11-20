//! Optional trusted ingress geography. Ordinary country headers never establish visitor location.
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, hash, process_config};
/// Whether a trusted ingress may tell the visitor country.
pub fn configured() -> bool {
    process_config::get().behavior_geo_secret.is_some()
}
/// The visitor country a trusted ingress proved; ordinary headers never count.
pub fn country(h: &RequestContext) -> Option<String> {
    let secret = process_config::get().behavior_geo_secret.as_deref()?;
    let supplied = header(h, "x-veyndo-geo-token")?;
    if hash(supplied) != hash(secret) {
        return None;
    }
    let country = header(h, "x-veyndo-geo-country")?;
    // Reuse the bundled world catalogue rather than accepting invented country codes.
    veyndo_shipping::domain::geography::world()
        .iter()
        .find(|c| c.iso_assigned && c.code == country)
        .map(|c| c.code.clone())
}
