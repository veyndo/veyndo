//! Contract of the pure methods of `veyndo_app_platform::ports::RecordRules`, and a test adapter for apps without rules of their own.
use serde_json::{Value, json};
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::ports::{RecordRules, RuleFuture};
use veyndo_core::Result;

/// An app with its own record rules: a field set its rules refuse.
pub struct Ruled<'a> {
    pub manifest: &'a Manifest,
    pub entity: &'a Entity,
    pub refused: Value,
}

/// Records of an app without rules are stored as given, admitted and have no planning format;
/// normalizing twice equals normalizing once. An app with rules refuses its refused fields and
/// publishes a planning format.
pub fn pure_rules(rules: &dyn RecordRules, plain: &Manifest, ruled: Option<Ruled<'_>>) {
    let inputs = [
        json!({"id":"r1","revision":1,"fields":{"title":"Kept","nested":{"n":1}}}),
        json!({"id":null,"fields":{}}),
        json!({}),
    ];
    for input in inputs {
        let once = rules
            .normalize(plain, &input)
            .expect("RecordRules: plain input refused");
        assert_eq!(
            once, input,
            "RecordRules: a plain app's record was rewritten"
        );
        let twice = rules
            .normalize(plain, &once)
            .expect("RecordRules: normalized input refused");
        assert_eq!(twice, once, "RecordRules: normalization is not idempotent");
        for entity in &plain.entities {
            assert!(
                rules
                    .check(plain, entity, input.get("fields").unwrap_or(&Value::Null))
                    .is_ok(),
                "RecordRules: a plain app's fields were refused"
            );
        }
    }
    assert!(
        rules.planning_format(plain).is_null(),
        "RecordRules: a plain app has a planning format"
    );
    if let Some(r) = ruled {
        assert!(
            rules.check(r.manifest, r.entity, &r.refused).is_err(),
            "RecordRules: the refused fields of {} were admitted",
            r.manifest.id
        );
        assert!(
            !rules.planning_format(r.manifest).is_null(),
            "RecordRules: {} has no planning format",
            r.manifest.id
        );
    }
}

/// The test adapter: no app has rules of its own.
pub struct NoRules;

impl RecordRules for NoRules {
    fn normalize(&self, _m: &Manifest, v: &Value) -> Result<Value> {
        Ok(v.clone())
    }
    fn check(&self, _m: &Manifest, _e: &Entity, _fields: &Value) -> Result<()> {
        Ok(())
    }
    fn planning_format(&self, _m: &Manifest) -> Value {
        Value::Null
    }
    fn validate<'a, 'c: 'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _t: &'a str,
        _m: &'a Manifest,
        _e: &'a Entity,
        _v: &'a Value,
    ) -> RuleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

/// A bundled manifest the contracts use as an app without rules: one with managed entities.
pub fn plain_manifest() -> Manifest {
    serde_json::from_str(include_str!("../../../extensions/apps/edi/manifest.json"))
        .expect("bundled EDI manifest")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_test_adapter_meets_the_contract() {
        super::pure_rules(&super::NoRules, &super::plain_manifest(), None);
    }
}
