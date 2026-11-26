//! Which language a request reads in and the fallback chain of its translations; the shop main language always closes the chain.
use crate::service::read_cache::LanguageRow;
use veyndo_core::{Result, bad};
use veyndo_pricing::domain::context::{Language, SYSTEM_LANGUAGE, language_chain};

/// The locale and translation chain of a request: an explicit language id wins, otherwise the requested locale or the main one.
pub fn select(
    rows: &[LanguageRow],
    main: &str,
    locale: Option<&str>,
    language_id: Option<&str>,
) -> Result<(String, Vec<String>)> {
    let locale = locale.unwrap_or(main);
    let selected = if let Some(id) = language_id {
        rows.iter().find(|r| r.id == id)
    } else {
        rows.iter().find(|r| r.locale == locale)
    }
    .ok_or(bad("Language unavailable"))?;
    let available = rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    let languages = rows
        .iter()
        .map(|r| Language {
            id: r.id.clone(),
            parent_id: r.parent_id.clone(),
        })
        .collect::<Vec<_>>();
    let mut chain = language_chain(&selected.id, &available, &languages).map_err(bad)?;
    if main != "en-GB" {
        let main_id = rows
            .iter()
            .find(|r| r.locale == main)
            .ok_or(bad("Main language unavailable"))?
            .id
            .clone();
        let current = selected.id.clone();
        chain.retain(|id| id != SYSTEM_LANGUAGE || id == &current);
        if !chain.contains(&main_id) {
            chain.push(main_id);
        }
    }
    Ok((selected.locale.clone(), chain))
}

#[cfg(test)]
mod tests {
    use super::select;
    use crate::service::read_cache::LanguageRow;
    use veyndo_pricing::domain::context::SYSTEM_LANGUAGE;

    const GERMAN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SWISS: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn rows() -> Vec<LanguageRow> {
        vec![
            LanguageRow {
                id: SYSTEM_LANGUAGE.into(),
                locale: "en-GB".into(),
                parent_id: None,
            },
            LanguageRow {
                id: GERMAN.into(),
                locale: "de-DE".into(),
                parent_id: None,
            },
            LanguageRow {
                id: SWISS.into(),
                locale: "de-CH".into(),
                parent_id: Some(GERMAN.into()),
            },
        ]
    }

    #[test]
    fn an_english_main_shop_falls_back_to_the_system_language() {
        let (locale, chain) = select(&rows(), "en-GB", Some("de-CH"), None).unwrap();
        assert_eq!(locale, "de-CH");
        assert_eq!(
            chain,
            vec![SWISS.to_string(), GERMAN.into(), SYSTEM_LANGUAGE.into()]
        );
        let (locale, chain) = select(&rows(), "en-GB", None, None).unwrap();
        assert_eq!(locale, "en-GB");
        assert_eq!(chain, vec![SYSTEM_LANGUAGE.to_string()]);
    }

    #[test]
    fn another_main_language_replaces_the_system_fallback() {
        let (locale, chain) = select(&rows(), "de-DE", None, None).unwrap();
        assert_eq!(locale, "de-DE");
        assert_eq!(chain, vec![GERMAN.to_string()]);
        let (_, chain) = select(&rows(), "de-DE", None, Some(SYSTEM_LANGUAGE)).unwrap();
        assert_eq!(chain, vec![SYSTEM_LANGUAGE.to_string(), GERMAN.into()]);
    }

    #[test]
    fn unknown_languages_are_refused() {
        assert!(select(&rows(), "en-GB", Some("fr-FR"), None).is_err());
        assert!(
            select(
                &rows(),
                "en-GB",
                None,
                Some("cccccccccccccccccccccccccccccccc")
            )
            .is_err()
        );
        assert!(select(&rows(), "fr-FR", Some("de-DE"), None).is_err());
    }
}
