//! The address aggregate is restored by the customers component; the shop's geography admits each address here.
use super::*;
pub(crate) async fn restore_book(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    state: &Value,
    s: &commerce::Settings,
) -> Result<()> {
    veyndo_customers::store::address_restore::restore(conn, t, email, state, &|address| {
        commerce::validate_address_geography(address, s)?;
        if !s.countries.contains(&address.country) {
            return Err(bad("Historical address country is unavailable"));
        }
        Ok(())
    })
    .await
}
