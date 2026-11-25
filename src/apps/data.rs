//! Managed app records live in the apps component; this module supplies the record rules of the bundled site builder.
use super::{Entity, Manifest, site_builder};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_app_platform::ports::{RecordRules, RuleFuture};
use veyndo_app_platform::store::managed_data;
use veyndo_core::Result;
use veyndo_core::document::{lenient, view};
/// A site builder record: its id and the app-declared fields the site builder validates.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SiteRecord {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    fields: Value,
}
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
                let record: SiteRecord = view(v);
                site_builder::validate(&e.name, &record.fields)?;
                site_builder::validate_owned(
                    tx,
                    t,
                    &record.fields,
                    record.id.as_deref().unwrap_or(""),
                )
                .await?;
            }
            Ok(())
        })
    }
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
/// A managed record save: its ID, revision and fields.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RecordInput {
    id: Value,
    revision: Value,
    fields: Value,
}

/// Every generic import and ordinary save uses the same app-owned normalization.
pub(crate) fn normalize_input(m: &Manifest, v: &Value) -> Result<Value> {
    let input: RecordInput = view(v);
    if m.id == "site_builder" {
        Ok(
            json!({"id":input.id,"revision":input.revision,"fields":site_builder::normalize(&input.fields)?}),
        )
    } else {
        Ok(v.clone())
    }
}
