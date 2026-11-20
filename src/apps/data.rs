//! Managed app records live in the apps component; this module supplies the record rules of the bundled site builder.
use super::*;
use veyndo_app_platform::ports::{RecordRules, RuleFuture};
use veyndo_app_platform::store::managed_data;
pub(crate) use veyndo_app_platform::store::managed_data::{list_page, record_revision};
/// The bundled site builder normalizes and checks its own pages; every other app has no extra rules.
pub(crate) struct BundledRules;
pub(crate) static BUNDLED: BundledRules = BundledRules;
impl RecordRules for BundledRules {
    fn normalize(&self, m: &Manifest, v: &Value) -> Result<Value> {
        normalize_input(m, v)
    }
    fn check(&self, m: &Manifest, e: &Entity, fields: &Value) -> Result<()> {
        if m.id == "site_builder" {
            site_builder::validate(&e.name, fields)?;
        }
        Ok(())
    }
    fn planning_format(&self, m: &Manifest) -> Value {
        if m.id == "site_builder" {
            site_builder::planning_contract()
        } else {
            Value::Null
        }
    }
    fn validate<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        m: &'a Manifest,
        e: &'a Entity,
        v: &'a Value,
    ) -> RuleFuture<'a> {
        Box::pin(async move {
            if m.id == "site_builder" {
                site_builder::validate(&e.name, &v["fields"])?;
                site_builder::validate_owned(tx, t, &v["fields"], v["id"].as_str().unwrap_or(""))
                    .await?;
            }
            Ok(())
        })
    }
}
pub(crate) fn fields(e: &Entity, v: &Value) -> Result<()> {
    field_values::validate(e, v)
}
pub(crate) async fn save(
    a: &App,
    t: &str,
    m: &Manifest,
    e: &Entity,
    v: &Value,
    h: &RequestContext,
) -> Result<Value> {
    managed_data::save(&service_limits::platform(a), t, m, e, v, h).await
}
pub(crate) async fn save_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
    e: &Entity,
    v: &Value,
) -> Result<Value> {
    managed_data::save_tx(&BUNDLED, tx, t, m, e, v).await
}
/// Every generic import and ordinary save uses the same app-owned normalization.
pub(crate) fn normalize_input(m: &Manifest, v: &Value) -> Result<Value> {
    if m.id == "site_builder" {
        Ok(
            json!({"id":v["id"],"revision":v["revision"],"fields":site_builder::normalize(&v["fields"])?}),
        )
    } else {
        Ok(v.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fields_reject_type_and_unknown_properties() {
        let e = Entity {
            operational: None,
            extends: None,
            name: "config".into(),
            label: HashMap::new(),
            public_read: false,
            publication: None,
            workflow: None,
            fields: vec![Field {
                name: "fee".into(),
                translatable: false,
                label: HashMap::new(),
                kind: "integer".into(),
                required: true,
                indexed: false,
                references: None,
                core_reference: None,
                choices: vec![],
                unique: false,
                validation: None,
            }],
        };
        assert!(fields(&e, &json!({"fee":200})).is_ok());
        assert!(fields(&e, &json!({"fee":"200"})).is_err());
        assert!(fields(&e, &json!({"fee":200,"tenant":"other"})).is_err());
    }
}
