//! Enabled content languages, NULL field inheritance and stable global language registration.
use super::*;
pub(crate) use veyndo_core::localized::valid_locale_key;

pub(crate) fn key(locale: &str, s: &Settings) -> String {
    s.languages().key(locale)
}
pub(crate) fn allowed(key: &str, s: &Settings) -> bool {
    s.languages().allowed(key)
}
pub(super) async fn register(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    s: &Settings,
) -> Result<()> {
    for locale in &s.locales {
        sqlx::query("SELECT public.veyndo_register_language($1)")
            .bind(locale)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
