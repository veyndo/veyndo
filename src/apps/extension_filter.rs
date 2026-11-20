//! Exact app-field predicates validated against current packages and applied before native pagination.
use super::{Manifest, core_extensions::Object, editor_contract};
use crate::{App, RequestContext, Result, bad, merchant};
pub(crate) use veyndo_app_platform::domain::extension_filter::{Filter, decode};
pub(crate) use veyndo_app_platform::store::extension_filter::predicate;

pub(crate) async fn admit(
    a: &App,
    h: &RequestContext,
    object: Object,
    input: &str,
) -> Result<Vec<Filter>> {
    let mut filters = decode(input)?;
    if filters.is_empty() {
        return Ok(filters);
    }
    let t = merchant(a, h)?;
    for f in &mut filters {
        let m: Manifest = super::package(a, &t, &f.app, true).await?;
        let e = m
            .entities
            .iter()
            .find(|e| e.name == f.entity)
            .ok_or(bad("Unknown extension model"))?;
        let x = e
            .extends
            .as_ref()
            .filter(|x| x.object == object)
            .ok_or(bad("App model does not extend this native object"))?;
        editor_contract::entity_access(&m, e, h, "list")?;
        let field = e
            .fields
            .iter()
            .find(|field| field.name == f.field && field.indexed && !field.translatable)
            .ok_or(bad(
                "Only declared indexed non-translated app fields are searchable",
            ))?;
        if !match field.kind.as_str() {
            "string" => f.value.is_string(),
            "integer" => f.value.is_i64(),
            "boolean" => f.value.is_boolean(),
            _ => false,
        } {
            return Err(bad("App filter type does not match the installed model"));
        }
        f.bind(
            x.field.clone(),
            crate::hash(&serde_json::json!(m).to_string()),
        );
    }
    Ok(filters)
}
