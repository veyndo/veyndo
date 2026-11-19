//! Country catalogue of a shop; the bundled world and the admission rules are owned by the shipping component.
use super::*;
use veyndo_shipping::domain::geography;
pub(crate) use veyndo_shipping::domain::geography::Country;
pub(crate) fn catalogue(s: &Settings) -> Vec<Country> {
    geography::catalogue(&s.country_definitions)
}
pub(crate) async fn country_catalogue(
    State(a): State<App>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let (s, revision) = config(&a, &tenant(&h)?).await?;
    Ok(Json(
        json!({"countries":catalogue(&s),"enabled":s.countries,"mainLocale":s.main_locale,"locales":s.locales,"revision":revision}),
    ))
}
/// All address entry points share tenant-defined country/subdivision validation.
pub(crate) fn validate_address_geography(address: &Address, settings: &Settings) -> Result<()> {
    geography::validate_address(
        &address.country,
        &address.country_state_id,
        &settings.country_definitions,
    )
}
