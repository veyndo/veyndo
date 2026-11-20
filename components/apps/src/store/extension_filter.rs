//! Admitted app-field predicates applied before native pagination.
use crate::domain::extension_filter::Filter;
use veyndo_app_model::Object;

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
