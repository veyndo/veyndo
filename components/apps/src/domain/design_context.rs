//! Editor context is validated against the supplied Manifest; references are untrusted evidence, never authority.
//! The editor context is typed as the host checks it; references and the manifest stay app-declared open JSON,
//! checked against the typed `Manifest`.
use crate::domain::draft_document::{Identified, elements, write};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad, conflict, hash};

/// The generation request members the editor context is checked against.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Request {
    context: Option<Value>,
    manifest: Value,
}

/// The editor selection a generation request carries; members read as `None` when they are not strings.
#[derive(Deserialize, Default)]
#[serde(default)]
struct EditorContext {
    #[serde(deserialize_with = "lenient")]
    view: Option<String>,
    #[serde(deserialize_with = "lenient")]
    selected: Option<String>,
    #[serde(deserialize_with = "lenient")]
    language: Option<String>,
    channel: Value,
}

/// The manifest views the selection must still name.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ManifestViews {
    views: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DraftView {
    id: Value,
    blocks: Value,
}

/// Reject stale element/language scope before contacting a provider or external reference.
pub fn validate(v: &Value, locales: &[String]) -> Result<Value> {
    let request: Request = view(v);
    let Some(context) = request.context.filter(|c| !c.is_null()) else {
        return Ok(Value::Null);
    };
    let context: EditorContext = view(&context);
    let view_id = context.view.as_deref().ok_or(bad("Select an app view"))?;
    let selected = context
        .selected
        .as_deref()
        .ok_or(bad("Invalid selected element"))?;
    let language = context
        .language
        .as_deref()
        .ok_or(bad("Select a content language"))?;
    if context.channel != "*" || !locales.iter().any(|l| l == language) {
        return Err(bad(
            "App definitions use all channels and a configured content language",
        ));
    }
    let views = view::<ManifestViews>(&request.manifest).views;
    let found = views.as_array().and_then(|rows| {
        rows.iter()
            .map(view::<DraftView>)
            .find(|row| row.id == view_id)
    });
    if (!view_id.is_empty() && found.is_none())
        || (!selected.is_empty()
            && !found
                .as_ref()
                .and_then(|row| row.blocks.as_array())
                .is_some_and(|rows| {
                    rows.iter()
                        .any(|row| view::<Identified>(row).id == selected)
                }))
    {
        return Err(conflict("Selected app view or element no longer exists"));
    }
    Ok(
        json!({"view":view_id,"selected":selected,"language":language,"channel":"*","manifestDigest":hash(&request.manifest.to_string())}),
    )
}

#[derive(Serialize)]
struct Digest {
    digest: String,
}

/// Persist only context, capture status and content digests, never uploaded pixels or fetched HTML.
pub fn receipt(context: &Value, evidence: &Value) -> Value {
    let sources: Vec<Value> = elements::<veyndo_core::document::Raw<()>>(evidence)
        .into_iter()
        .map(|source| {
            let mut source = source.raw;
            if let Some(text) = source.as_object_mut().and_then(|s| s.remove("text")) {
                write(
                    &mut source,
                    Digest {
                        digest: hash(&text.to_string()),
                    },
                )
                .expect("an evidence source with text is an object");
            }
            source
        })
        .collect();
    json!({"version":1,"context":context,"sources":sources})
}
/// The stored prompt of a build; design evidence travels with the instruction.
pub fn stored(prompt: &str, evidence: Option<&Value>) -> String {
    evidence.map_or_else(
        || prompt.into(),
        |e| json!({"designEvidence":e,"instruction":prompt}).to_string(),
    )
}
/// The design evidence a stored prompt carries, or null.
pub fn restored(prompt: &str) -> Value {
    serde_json::from_str::<Value>(prompt)
        .ok()
        .and_then(|v| v.get("designEvidence").cloned())
        .unwrap_or(Value::Null)
}
#[cfg(test)]
/// The untyped implementation this module replaced, kept as the oracle for the JSON equivalence test.
mod legacy {
    use serde_json::{Value, json};
    use veyndo_core::{Result, bad, conflict, hash};
    /// Reject stale element/language scope before contacting a provider or external reference.
    pub fn validate(v: &Value, locales: &[String]) -> Result<Value> {
        let Some(context) = v.get("context").filter(|c| !c.is_null()) else {
            return Ok(Value::Null);
        };
        let view = context["view"].as_str().ok_or(bad("Select an app view"))?;
        let selected = context["selected"]
            .as_str()
            .ok_or(bad("Invalid selected element"))?;
        let language = context["language"]
            .as_str()
            .ok_or(bad("Select a content language"))?;
        if context["channel"] != "*" || !locales.iter().any(|l| l == language) {
            return Err(bad(
                "App definitions use all channels and a configured content language",
            ));
        }
        let views = v["manifest"]["views"].as_array();
        let found = views.and_then(|rows| rows.iter().find(|row| row["id"] == view));
        if (!view.is_empty() && found.is_none())
            || (!selected.is_empty()
                && !found
                    .and_then(|row| row["blocks"].as_array())
                    .is_some_and(|rows| rows.iter().any(|row| row["id"] == selected)))
        {
            return Err(conflict("Selected app view or element no longer exists"));
        }
        Ok(
            json!({"view":view,"selected":selected,"language":language,"channel":"*","manifestDigest":hash(&v["manifest"].to_string())}),
        )
    }
    /// Persist only context, capture status and content digests, never uploaded pixels or fetched HTML.
    pub fn receipt(context: &Value, evidence: &Value) -> Value {
        let sources: Vec<Value> = evidence
            .as_array()
            .into_iter()
            .flatten()
            .map(|source| {
                let mut source = source.clone();
                if let Some(text) = source.as_object_mut().and_then(|s| s.remove("text")) {
                    source["digest"] = json!(hash(&text.to_string()));
                }
                source
            })
            .collect();
        json!({"version":1,"context":context,"sources":sources})
    }
}
#[cfg(test)]
mod tests {
    use super::{receipt, restored, stored, validate};
    use serde_json::{Value, json};
    #[test]
    fn selection_is_manifest_and_locale_bound() {
        let mut input = json!({"manifest":{"views":[{"id":"detail","blocks":[{"id":"buy"}]}]},"context":{"view":"detail","selected":"buy","language":"de-DE","channel":"*"}});
        assert!(validate(&input, &["de-DE".into()]).is_ok());
        input["context"]["selected"] = json!("foreign");
        assert!(validate(&input, &["de-DE".into()]).is_err());
        input["context"]["selected"] = json!("");
        assert!(validate(&input, &["fr-FR".into()]).is_err());
        input["context"]["channel"] = json!("other");
        assert!(validate(&input, &["de-DE".into()]).is_err());
    }
    #[test]
    fn retained_evidence_has_no_source_body() {
        let e = receipt(
            &Value::Null,
            &json!([{"url":"https://example.com","text":"<script>untrusted</script>","status":"partial"}]),
        );
        assert!(e["sources"][0].get("text").is_none());
        assert!(e["sources"][0]["digest"].is_string());
        assert_eq!(restored(&stored("test", Some(&e))), e);
        assert!(restored("Visual App Studio build").is_null());
    }
}
#[cfg(test)]
mod equivalence {
    use serde_json::{Value, json};

    #[test]
    fn typed_editor_context_and_receipt_are_unchanged() {
        let manifest = json!({"views":[{"id":"main","blocks":[{"id":"save"},"x"]},{"id":"","blocks":"none"},7]});
        let contexts = [
            Value::Null,
            json!({"view":"main","selected":"save","language":"en","channel":"*"}),
            json!({"view":"main","selected":"","language":"en","channel":"*"}),
            json!({"view":"","selected":"","language":"en","channel":"*"}),
            json!({"view":"","selected":"save","language":"en","channel":"*"}),
            json!({"view":"gone","selected":"","language":"en","channel":"*"}),
            json!({"view":"main","selected":"gone","language":"en","channel":"*"}),
            json!({"view":"main","selected":"save","language":"fr","channel":"*"}),
            json!({"view":"main","selected":"save","language":"en","channel":"web"}),
            json!({"view":1,"selected":"save","language":"en","channel":"*"}),
            json!({"view":"main","selected":null,"language":"en","channel":"*"}),
            json!({"view":"main","selected":"save","channel":"*"}),
            json!("text"),
        ];
        for context in contexts {
            for request in [
                json!({"context":context,"manifest":manifest}),
                json!({"context":context}),
                json!([context]),
            ] {
                match (
                    super::validate(&request, &["en".into()]),
                    super::legacy::validate(&request, &["en".into()]),
                ) {
                    (Ok(a), Ok(b)) => assert_eq!(a, b, "{request}"),
                    (Err(a), Err(b)) => assert_eq!((a.0, a.1), (b.0, b.1), "{request}"),
                    (a, b) => panic!("{request}: {:?} vs {:?}", a.is_ok(), b.is_ok()),
                }
            }
        }
        let evidence =
            json!([{"kind":"url","text":"<html>"},{"kind":"image"},"plain",{"text":null}]);
        for evidence in [evidence, Value::Null] {
            assert_eq!(
                super::receipt(&json!({"view":"main"}), &evidence),
                super::legacy::receipt(&json!({"view":"main"}), &evidence)
            );
        }
    }
}
