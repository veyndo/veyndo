//! Brand theme contract: the typed token document on a home page, vetted fonts and the WCAG AA contrast pairs every stored theme meets.
use serde::{Deserialize, Serialize};
use veyndo_core::{Result, bad};

/// Vetted heading/body font pairings; mirrors `fontPairings` in `frontend/src/kit/tokens/brand-theme.ts`.
const PAIRINGS: [(&str, &str, &str); 7] = [
    ("classic", "transitional", "neo-grotesque"),
    ("editorial", "didone", "neo-grotesque"),
    ("modern", "geometric", "neo-grotesque"),
    ("friendly", "humanist", "humanist"),
    ("crafted", "slab", "humanist"),
    ("timeless", "old-style", "old-style"),
    ("technical", "mono", "neo-grotesque"),
];
/// Motion intensities a shop can select.
pub const MOTION: [&str; 3] = ["off", "subtle", "expressive"];

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Palette {
    primary: String,
    secondary: Option<String>,
    neutral: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Colors {
    bg: String,
    surface: String,
    panel: String,
    hover: String,
    border: String,
    muted: String,
    text: String,
    accent: String,
    accent_text: String,
    accent_soft: String,
    accent_strong: String,
    accent_border: String,
    focus: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fonts {
    pairing: String,
    heading: String,
    body: String,
}
/// The stored brand token document (`theme.brand`).
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BrandTheme {
    version: u8,
    source: Vec<String>,
    palette: Palette,
    colors: Colors,
    fonts: Fonts,
    radius: String,
    density: String,
}

/// `#rrggbb` as sRGB channels; uppercase digits are refused so stored documents stay canonical.
fn channels(hex: &str) -> Option<[f64; 3]> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6
        || !digits
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    let n = u32::from_str_radix(digits, 16).ok()?;
    Some([(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|c| f64::from(c) / 255.0))
}
/// WCAG 2.x relative luminance.
fn luminance(rgb: [f64; 3]) -> f64 {
    let [r, g, b] = rgb.map(|c| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
/// WCAG 2.x contrast ratio of two colours, 1..21.
pub(crate) fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

impl Colors {
    fn get(&self, role: &str) -> &str {
        match role {
            "bg" => &self.bg,
            "surface" => &self.surface,
            "panel" => &self.panel,
            "hover" => &self.hover,
            "border" => &self.border,
            "muted" => &self.muted,
            "text" => &self.text,
            "accent" => &self.accent,
            "accentText" => &self.accent_text,
            "accentSoft" => &self.accent_soft,
            "accentStrong" => &self.accent_strong,
            "accentBorder" => &self.accent_border,
            _ => &self.focus,
        }
    }
}
/// The contrast pairs and minimum ratios; mirrors `contrastPairs` in the token package.
const PAIRS: [(&str, &str, f64); 17] = [
    ("text", "bg", 4.5),
    ("text", "surface", 4.5),
    ("text", "panel", 4.5),
    ("text", "hover", 4.5),
    ("text", "accentSoft", 4.5),
    ("muted", "bg", 4.5),
    ("muted", "surface", 4.5),
    ("muted", "panel", 4.5),
    ("accent", "bg", 4.5),
    ("accent", "surface", 4.5),
    ("accent", "panel", 4.5),
    ("accent", "accentSoft", 4.5),
    ("accentStrong", "surface", 4.5),
    ("accentText", "accent", 4.5),
    ("accentText", "accentStrong", 4.5),
    ("focus", "bg", 3.0),
    ("focus", "surface", 3.0),
];
const ROLES: [&str; 13] = [
    "bg",
    "surface",
    "panel",
    "hover",
    "border",
    "muted",
    "text",
    "accent",
    "accentText",
    "accentSoft",
    "accentStrong",
    "accentBorder",
    "focus",
];

/// Validates an untrusted brand document: shape, canonical colours, a vetted
/// pairing, known scales, bounded sources and every WCAG AA contrast pair.
pub(crate) fn validate(theme: &BrandTheme) -> Result<()> {
    if theme.version != 1
        || theme.source.len() > 6
        || theme
            .source
            .iter()
            .any(|s| s.is_empty() || s.len() > 200 || s.contains(['<', '>', '\n', '\r']))
    {
        return Err(bad("Invalid brand theme document"));
    }
    let palette = [
        Some(&theme.palette.primary),
        theme.palette.secondary.as_ref(),
        Some(&theme.palette.neutral),
    ];
    if palette.into_iter().flatten().any(|c| channels(c).is_none())
        || ROLES
            .iter()
            .any(|r| channels(theme.colors.get(r)).is_none())
    {
        return Err(bad("Brand theme colors require lowercase #rrggbb"));
    }
    let f = &theme.fonts;
    if !PAIRINGS
        .iter()
        .any(|(p, h, b)| *p == f.pairing && *h == f.heading && *b == f.body)
        || !["sharp", "soft", "round"].contains(&theme.radius.as_str())
        || !["compact", "regular", "airy"].contains(&theme.density.as_str())
    {
        return Err(bad("Brand theme fonts and scales must be vetted choices"));
    }
    for (fg, bg, min) in PAIRS {
        let (a, b) = (
            channels(theme.colors.get(fg)),
            channels(theme.colors.get(bg)),
        );
        if let (Some(a), Some(b)) = (a, b)
            && contrast(a, b) < min
        {
            return Err(bad("Brand theme colors miss WCAG AA contrast"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    /// The theme the token package derives for a fixed palette (written by `brand-theme.test.ts`).
    fn fixture() -> Value {
        serde_json::from_str(include_str!("tests/brand-theme-fixture.json")).unwrap()
    }
    fn check(v: &Value) -> Result<()> {
        let theme: BrandTheme = serde_json::from_value(v.clone()).map_err(|_| bad("shape"))?;
        validate(&theme)
    }
    #[test]
    fn derived_themes_pass_and_every_pair_is_enforced() {
        let all = fixture();
        for theme in all.as_object().unwrap().values() {
            check(theme).unwrap();
        }
        let base = all["vivid"].clone();
        for (fg, bg, _) in PAIRS {
            let mut v = base.clone();
            // The same colour on both sides is contrast 1:1.
            let other = v["colors"][bg].clone();
            v["colors"][fg] = other;
            assert!(check(&v).is_err(), "{fg} on {bg} must be refused");
        }
    }
    #[test]
    fn untrusted_shapes_and_choices_are_refused() {
        let base = fixture()["vivid"].clone();
        let cases = [
            ("/fonts/heading", json!("comic")),
            ("/fonts/pairing", json!("modern")),
            ("/radius", json!("blob")),
            ("/density", json!(9)),
            ("/version", json!(2)),
            ("/colors/accent", json!("#ABCDEF")),
            ("/colors/accent", json!("url(javascript:alert(1))")),
            ("/source", json!(["<script>"])),
            ("/source", json!(["a", "b", "c", "d", "e", "f", "g"])),
        ];
        for (path, value) in cases {
            let mut v = base.clone();
            *v.pointer_mut(path).unwrap() = value;
            assert!(check(&v).is_err(), "{path} must be refused");
        }
        let mut extra = base.clone();
        extra["css"] = json!("body{}");
        assert!(check(&extra).is_err());
    }
    #[test]
    fn contrast_matches_wcag_reference_values() {
        let (black, white) = ([0.0; 3], [1.0; 3]);
        assert!((contrast(black, white) - 21.0).abs() < 1e-9);
        assert!((contrast(white, white) - 1.0).abs() < 1e-9);
        // #767676 on white is the classic 4.54:1 boundary grey.
        let grey = channels("#767676").unwrap();
        assert!((contrast(grey, white) - 4.54).abs() < 0.01);
    }
}
