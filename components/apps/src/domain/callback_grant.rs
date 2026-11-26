//! What an app callback key may do: the capability the merchant approved and the role scope behind it.
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_identity::permit as role;
use veyndo_kernel::verified_kernel;

/// Callback consent is an app capability, not merely its mapped native role scope.
pub fn permit(h: &RequestContext, capability: &str) -> Result<()> {
    let grants = h
        .principal
        .app
        .as_ref()
        .and(h.principal.app_permissions.as_deref())
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());
    let (capability_approved, role_approved) = veyndo_app_model::admission::callback_facts(
        h.principal.app.is_some(),
        grants.as_deref(),
        capability,
        |scope| role(h, scope).is_ok(),
    );
    if !verified_kernel::app_callback_admissible(capability_approved, role_approved) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "App callback capability was not approved for this key".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::permit;
    use veyndo_core::RequestContext;

    #[test]
    fn a_callback_key_needs_the_approved_capability_and_its_role_scope() {
        let mut h = RequestContext::new();
        h.insert("x-rac-role", "admin".parse().unwrap());
        h.principal.app = Some("sample".into());
        h.principal.app_permissions = Some(r#"["data.read"]"#.into());
        assert!(permit(&h, "data.read").is_ok());
        assert!(permit(&h, "orders.read").is_err());
        h.principal.app_permissions = None;
        assert!(permit(&h, "data.read").is_err());
    }
}
