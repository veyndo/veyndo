//! Which language keys a shop may use for content: canonical language, script, region and variant subtags only.
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
