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

#[cfg(test)]
mod tests {
    use super::{admit_guidance, validate};
    use serde_json::{Value, json};
    use veyndo_core::{RequestContext, StatusCode};

    fn check(v: Value) -> veyndo_core::Result<Value> {
        validate(&["en-GB".into(), "de-DE".into()], "en-GB", &v)
    }

    #[test]
    fn a_source_is_returned_in_canonical_form_with_a_digest_of_its_content() {
        let v = check(json!({"title":"Care","content":"Wash cold","productId":""})).unwrap();
        assert_eq!(v["kind"], "document");
        assert_eq!(v["locale"], "en-GB");
        assert_eq!(v["productId"], Value::Null);
        assert_eq!(v["translations"], json!({}));
        let same = check(json!({"title":"Other title","content":"Wash cold"})).unwrap();
        assert_eq!(v["digest"], same["digest"]);
        let translated = check(
            json!({"title":"Care","content":"Wash cold","kind":"care","productId":"p1",
            "translations":{"de-DE":{"title":"Pflege","content":null}}}),
        )
        .unwrap();
        assert_ne!(v["digest"], translated["digest"]);
        assert_eq!(translated["productId"], "p1");
        assert_eq!(translated["kind"], "care");
    }

    #[test]
    fn unknown_fields_languages_kinds_and_oversized_text_are_refused() {
        for invalid in [
            json!("text"),
            json!({"title":"Care","content":"Wash cold","owner":"x"}),
            json!({"title":"Care","content":"Wash cold","locale":"fr-FR"}),
            json!({"title":" ","content":"Wash cold"}),
            json!({"title":"x".repeat(201),"content":"Wash cold"}),
            json!({"title":"Care","content":""}),
            json!({"title":"Care","content":"x".repeat(100_001)}),
            json!({"title":"Care","content":"Wash cold","kind":"secret"}),
            json!({"title":"Care","content":"Wash cold","translations":[]}),
            json!({"title":"Care","content":"Wash cold","translations":{"en-GB":{"title":"Care"}}}),
            json!({"title":"Care","content":"Wash cold","translations":{"fr-FR":{"title":"Soin"}}}),
            json!({"title":"Care","content":"Wash cold","translations":{"de-DE":"Pflege"}}),
            json!({"title":"Care","content":"Wash cold","translations":{"de-DE":{"summary":"x"}}}),
            json!({"title":"Care","content":"Wash cold","translations":{"de-DE":{"title":" "}}}),
            json!({"title":"Care","content":"Wash cold","translations":{"de-DE":{"title":7}}}),
            json!({"title":"Care","content":"Wash cold","productId":7}),
        ] {
            assert!(
                check(invalid.clone()).is_err(),
                "{}",
                invalid.to_string().chars().take(120).collect::<String>()
            );
        }
    }

    #[test]
    fn guidance_is_written_only_by_a_personal_merchant_session() {
        let mut h = RequestContext::new();
        assert_eq!(
            admit_guidance(&h, &json!("guidance")).err().map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        assert!(admit_guidance(&h, &json!("manual")).is_ok());
        h.principal.personal_session = true;
        assert!(admit_guidance(&h, &json!("guidance")).is_ok());
        h.principal.app = Some("email".into());
        assert!(admit_guidance(&h, &json!("guidance")).is_err());
    }
}
