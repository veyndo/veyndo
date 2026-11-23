//! Admitted app-field predicates applied before native pagination.
use crate::domain::entity_access::entity_access;
use crate::domain::extension_filter::{Filter, decode};
use crate::store::packages::package;
use veyndo_app_model::Object;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_identity::merchant;

/// Admits the app-field filters of a list request against the current packages: declared, indexed, untranslated, readable and typed.
pub async fn admit(
    a: &State,
    h: &RequestContext,
    object: Object,
    input: &str,
) -> Result<Vec<Filter>> {
    let mut filters = decode(input)?;
    if filters.is_empty() {
        return Ok(filters);
    }
    let t = merchant(h)?;
    for f in &mut filters {
        let m: Manifest = package(a, &t, &f.app, true).await?;
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
        entity_access(&m, e, h, "list")?;
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
        f.bind(x.field.clone(), hash(&serde_json::json!(m).to_string()));
    }
    Ok(filters)
}

/// Adds one EXISTS predicate per admitted filter to a native list query.
pub fn predicate(
    q: &mut sqlx::QueryBuilder<sqlx::Postgres>,
    object: Object,
    t: &str,
    filters: &[Filter],
) {
    let (column, id) = match object {
        Object::Product => ("product_id", "p.id"),
        Object::Order => ("order_id", "o.id"),
        Object::Customer => ("customer_id", "contact->>'id'"),
        Object::Company => ("company_id", "id"),
    };
    for f in filters {
        q.push(" AND EXISTS(SELECT 1 FROM app_record_references xr JOIN app_record_indexes xi ON xi.tenant=xr.tenant AND xi.app=xr.app AND xi.entity=xr.entity AND xi.record_id=xr.record_id JOIN app_packages xp ON xp.tenant=xr.tenant AND xp.id=xr.app AND xp.active WHERE xr.tenant=").push_bind(t.to_owned())
            .push(" AND xp.digest=").push_bind(f.digest.clone())
            .push(" AND xr.app=").push_bind(f.app.clone()).push(" AND xr.entity=").push_bind(f.entity.clone())
            .push(" AND xr.field=").push_bind(f.anchor.clone()).push(" AND xr.").push(column).push("=").push(id)
            .push(" AND xi.field=").push_bind(f.field.clone())
            .push(" AND xi.value_hash=sha256(jsonb_send(").push_bind(f.value.clone()).push("::jsonb)) AND xi.value=").push_bind(f.value.clone()).push(")");
    }
}
