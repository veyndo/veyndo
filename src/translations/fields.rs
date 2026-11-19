//! Translation field extraction and admission are owned by the catalog component.
use super::*;
use veyndo_catalog::domain::translation_fields::Source;
pub(super) use veyndo_catalog::domain::translation_fields::{texts, translated};
pub(super) fn source(row: &sqlx::postgres::PgRow, locale: &str, main: &str) -> Value {
    veyndo_catalog::domain::translation_fields::source(
        Source {
            name: row.get("name"),
            description: row.get("description"),
            translated_name: row.get("translated_name"),
            translated_description: row.get("translated_description"),
            extra: row.get("extra"),
        },
        locale,
        main,
    )
}
pub(super) fn key(locale: &str, locales: &[String]) -> String {
    veyndo_core::localized::Languages {
        main: locale,
        enabled: locales,
    }
    .key(locale)
}
