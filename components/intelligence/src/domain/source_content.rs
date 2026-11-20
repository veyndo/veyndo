//! Bounds of a knowledge source: enabled languages, kinds, translated fields and the digest that binds its chunks.
use serde_json::{Value, json};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};

/// Admits a source in the shop's enabled languages and returns it in canonical form with its content digest.
pub fn validate(locales: &[String], main_locale: &str, v: &Value) -> Result<Value> {
    if v.as_object().is_none_or(|o| {
        o.keys().any(|k| {
            ![
                "title",
                "content",
                "productId",
                "kind",
                "locale",
                "translations",
                "revision",
            ]
            .contains(&k.as_str())
        })
    }) {
        return Err(bad("Unsupported source field"));
    }
    let locale = v["locale"].as_str().unwrap_or(main_locale);
    if !locales.iter().any(|l| l == locale) {
        return Err(bad("Source language must be enabled"));
    }
    let title = v["title"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 200)
        .ok_or(bad("Title required, maximum 200 bytes"))?;
    let text = v["content"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 100_000)
        .ok_or(bad("Document text must be 1..100000 UTF-8 bytes"))?;
    let kind = v["kind"].as_str().unwrap_or("document");
    if ![
        "document",
        "datasheet",
        "manual",
        "care",
        "faq",
        "shipping",
        "returns",
        "warranty",
        "brand",
        "guidance",
    ]
    .contains(&kind)
    {
        return Err(bad("Unknown knowledge kind"));
    }
    let translations = v.get("translations").cloned().unwrap_or(json!({}));
    let map = translations
        .as_object()
        .ok_or(bad("Translations must be an object"))?;
    for (l, fields) in map {
        if l == locale || !locales.contains(l) {
            return Err(bad(
                "Translation language must be enabled and different from source",
            ));
        }
        let fields = fields.as_object().ok_or(bad("Invalid translation"))?;
        for (key, value) in fields {
            let limit = match key.as_str() {
                "title" => 200,
                "content" => 100_000,
                _ => return Err(bad("Unknown translated field")),
            };
            if !value.is_null()
                && value
                    .as_str()
                    .is_none_or(|s| s.len() > limit || (key == "title" && s.trim().is_empty()))
            {
                return Err(bad("Invalid translated source value"));
            }
        }
    }
    if translations.to_string().len() + text.len() > 200_000 {
        return Err(bad("Combined source content exceeds 200000 bytes"));
    }
    if !v["productId"].is_null() && v["productId"].as_str().is_none() {
        return Err(bad("Invalid product reference"));
    }
    let digest = if map.is_empty() {
        hash(text)
    } else {
        hash(&format!("{locale}:{text}:{translations}"))
    };
    Ok(
        json!({"title":title,"content":text,"productId":v["productId"].as_str().filter(|s|!s.is_empty()),"kind":kind,"locale":locale,"translations":translations,"digest":digest}),
    )
}

/// Remembered guidance is a personal merchant decision; apps and integration keys cannot write it.
pub fn admit_guidance(h: &RequestContext, kind: &Value) -> Result<()> {
    if kind == "guidance" && (!h.principal.personal_session || h.principal.app.is_some()) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Remembered guidance requires a personal merchant decision".into(),
        ));
    }
    Ok(())
}
