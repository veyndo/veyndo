//! Package approval binds all requested permissions to a reviewed digest; current actor rights apply before every installation path.
use crate::domain::approval;
use serde_json::Value;
use veyndo_app_model::admission::callback_scope;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::permit;

/// Requires the actor to hold every native right the package asks to use.
pub fn actor_permissions(h: &RequestContext, m: &Manifest) -> Result<()> {
    if !m.lifecycles.is_empty() {
        permit(h, "settings.write")?;
        for c in &m.lifecycles {
            permit(
                h,
                if c.object == "order" {
                    "orders.write"
                } else {
                    "b2b.manage"
                },
            )?;
            for edge in &c.transitions {
                for permission in &edge.required_permissions {
                    permit(h, permission)?;
                }
            }
        }
    }
    for p in &m.permissions {
        if let Some(scope) = callback_scope(p) {
            permit(h, scope)?;
        }
        if p == "knowledge.write" {
            permit(h, "apps.manage")?;
        }
        if p == "payments.instructions.read" {
            for scope in ["payments.read", "orders.read", "customers.pii"] {
                permit(h, scope)?;
            }
        }
        if p == "payments.provider" {
            permit(h, "payments.manage")?;
        }
        if p == "commerce.hooks" {
            permit(h, "catalog.write")?;
            permit(h, "settings.write")?;
        }
    }
    Ok(())
}
/// Requires an approval of exactly the current digest and the complete permission list.
pub fn approved(h: &RequestContext, m: &Manifest, v: &Value) -> Result<()> {
    actor_permissions(h, m)?;
    let mut expected = m.permissions.clone();
    expected.sort();
    expected.dedup();
    let Some(requested) = v["permissions"].as_array() else {
        return Err(conflict("Review and approve the package permissions"));
    };
    let mut actual = requested
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .ok_or(bad("Invalid consent permissions"))?;
    actual.sort();
    if v["approve"] != true || v["digest"] != approval::canonical_digest(m) || actual != expected {
        return Err(conflict(
            "Package approval must match its current digest and complete permission list",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{actor_permissions, approved};
    use crate::domain::approval::{built_in, canonical_digest};
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::RequestContext;

    fn admin() -> RequestContext {
        let mut h = RequestContext::new();
        h.insert("x-rac-role", "admin".parse().unwrap());
        h
    }

    #[test]
    fn approval_needs_the_current_digest_and_the_complete_permission_list() {
        let m: Manifest = serde_json::from_str(built_in("slack").unwrap()).unwrap();
        let h = admin();
        assert!(actor_permissions(&h, &m).is_ok());
        let digest = canonical_digest(&m);
        let ok = json!({"approve":true,"digest":digest,"permissions":m.permissions});
        assert!(approved(&h, &m, &ok).is_ok());
        for wrong in [
            json!({"approve":false,"digest":digest,"permissions":m.permissions}),
            json!({"approve":true,"digest":"other","permissions":m.permissions}),
            json!({"approve":true,"digest":digest,"permissions":[]}),
            json!({"approve":true,"digest":digest}),
            json!({"approve":true,"digest":digest,"permissions":[1]}),
        ] {
            assert!(approved(&h, &m, &wrong).is_err(), "{wrong}");
        }
    }
}
