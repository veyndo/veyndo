//! A translated name and description as stored per content language; a missing field inherits from the shop's main language.
use serde::{Deserialize, Serialize};

pub mod languages;
pub use languages::{Languages, valid_locale_key};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}
