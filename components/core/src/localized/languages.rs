//! The content languages a shop enabled: which keys content may use, and which key a language is stored under.
use crate::{Result, bad};
use serde_json::Value;

/// The main language and all enabled content languages of one shop.
#[derive(Clone, Copy)]
pub struct Languages<'a> {
    pub main: &'a str,
    pub enabled: &'a [String],
}

impl<'a> Languages<'a> {
    /// The key content of `locale` is stored under: the base language while it is the only enabled variant, else the full locale.
    pub fn key(&self, locale: &str) -> String {
        let base = locale.split('-').next().unwrap_or(locale);
        if self
            .enabled
            .iter()
            .filter(|l| l.split('-').next() == Some(base))
            .count()
            == 1
        {
            base.into()
        } else {
            locale.into()
        }
    }
    /// The enabled language a content key refers to.
    pub fn locale(&self, key: &str) -> Option<&'a str> {
        self.enabled
            .iter()
            .find(|v| *v == key || self.key(v) == key)
            .map(String::as_str)
    }
    pub fn allowed(&self, key: &str) -> bool {
        self.locale(key).is_some()
    }
    /// A map of translated names: only enabled languages, each `null` (inherit) or non-empty up to `max`, and the main language set.
    pub fn validate_names(&self, names: &Value, max: usize) -> Result<()> {
        let map = names
            .as_object()
            .filter(|m| !m.is_empty() && m.len() <= 100)
            .ok_or(bad("Translated names required"))?;
        for (key, value) in map {
            if !self.allowed(key)
                || !value.is_null() && value.as_str().is_none_or(|s| s.is_empty() || s.len() > max)
            {
                return Err(bad("Invalid translated name"));
            }
        }
        if names[self.main]
            .as_str()
            .or_else(|| names[self.main.split('-').next().unwrap_or(self.main)].as_str())
            .is_none_or(|s| s.is_empty())
        {
            return Err(bad("Main language name required"));
        }
        Ok(())
    }
}

/// `true` for keys such as `en`, `zh-Hans-CN` or `es-419`; arbitrary subtags are refused.
pub fn valid_locale_key(v: &str) -> bool {
    let parts = v.split('-').collect::<Vec<_>>();
    if !(2..=3).contains(&parts[0].len())
        || !parts[0].bytes().all(|c| c.is_ascii_lowercase())
        || parts.len() > 3
    {
        return false;
    }
    // Canonical language[-Script][-REGION][-variant] subset: not arbitrary subtags.
    // Invalid regions such as en-12/en-USA make Intl.DisplayNames throw in editors.
    let mut tail = &parts[1..];
    if let Some(p) = tail.first()
        && p.len() == 4
        && p.bytes().all(|c| c.is_ascii_alphabetic())
    {
        if !p.as_bytes()[0].is_ascii_uppercase()
            || !p.bytes().skip(1).all(|c| c.is_ascii_lowercase())
        {
            return false;
        }
        tail = &tail[1..];
    }
    if let Some(p) = tail.first()
        && ((p.len() == 2 && p.bytes().all(|c| c.is_ascii_uppercase()))
            || (p.len() == 3 && p.bytes().all(|c| c.is_ascii_digit())))
    {
        tail = &tail[1..];
    }
    let mut seen = std::collections::HashSet::new();
    tail.iter().all(|p| {
        seen.insert(*p)
            && ((5..=8).contains(&p.len()) || (p.len() == 4 && p.as_bytes()[0].is_ascii_digit()))
            && p.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::Languages;
    use serde_json::json;

    fn enabled(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|c| (*c).to_owned()).collect()
    }

    #[test]
    fn a_language_is_stored_under_its_base_until_a_second_variant_is_enabled() {
        let one = enabled(&["es-ES", "de-DE"]);
        let languages = Languages {
            main: "es-ES",
            enabled: &one,
        };
        assert_eq!(languages.key("de-DE"), "de");
        assert_eq!(languages.locale("de"), Some("de-DE"));
        assert_eq!(languages.locale("de-DE"), Some("de-DE"));
        assert!(!languages.allowed("fr"));
        let two = enabled(&["es-ES", "de-DE", "de-AT"]);
        let languages = Languages {
            main: "es-ES",
            enabled: &two,
        };
        assert_eq!(languages.key("de-AT"), "de-AT");
        assert!(!languages.allowed("de"));
        assert!(languages.allowed("de-AT"));
    }

    #[test]
    fn names_need_the_main_language_and_only_enabled_languages() {
        let list = enabled(&["es-ES", "de-DE", "ja-JP"]);
        let languages = Languages {
            main: "es-ES",
            enabled: &list,
        };
        assert!(
            languages
                .validate_names(&json!({"es":"Nombre","de":null,"ja":"名前"}), 100)
                .is_ok()
        );
        assert!(
            languages
                .validate_names(&json!({"es-ES":"Nombre"}), 100)
                .is_ok()
        );
        for names in [
            json!({"de":"Name"}),
            json!({"es":"Nombre","fr":"Nom"}),
            json!({"es":"Nombre","de":""}),
            json!({"es":"N".repeat(101)}),
            json!({}),
            json!("Nombre"),
        ] {
            assert!(languages.validate_names(&names, 100).is_err(), "{names}");
        }
    }
}

#[cfg(test)]
mod key_tests {
    #[test]
    fn locale_keys_have_valid_canonical_subtags() {
        for key in [
            "en",
            "it-IT",
            "zh-Hans-CN",
            "es-419",
            "de-1901",
            "sl-rozaj-biske",
        ] {
            assert!(super::valid_locale_key(key), "{key}");
        }
        for key in [
            "",
            "en-12",
            "en-USA",
            "en-AB-CD",
            "en-US-US",
            "en-abcde-abcde",
            "en-us",
            "zh-hans-CN",
        ] {
            assert!(!super::valid_locale_key(key), "{key}");
        }
    }
}
