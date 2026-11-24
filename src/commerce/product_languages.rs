//! Enabled content languages, NULL field inheritance and stable global language registration.
use super::*;
pub(crate) use veyndo_core::localized::valid_locale_key;

pub(crate) fn key(locale: &str, s: &Settings) -> String {
    s.languages().key(locale)
}
