//! Identity component: the checks every other component uses to ask who a request acts for and what it may do.
pub mod domain;
pub mod route_policy;
pub use domain::permissions::{SCOPES, allowed, scope, validate_permissions};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_kernel::verified_kernel;

/// Refuses a request that has no signed-in identity or lacks the named permission.
pub fn permit(h: &RequestContext, kind: &str) -> Result<()> {
    header(h, "x-rac-role").ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Personal login or instance administrator credential required".into(),
    ))?;
    if allowed(h, kind) {
        Ok(())
    } else {
        Err(Error::permission(scope(kind)))
    }
}

/// The shop a signed-in merchant request acts for; the credential's shop must match the resolved one.
pub fn merchant(h: &RequestContext) -> Result<String> {
    permit(h, "read")?;
    let credential = header(h, "x-rac-tenant");
    let resolved = match credential {
        Some(_) => Some(tenant(h)?),
        None => None,
    };
    if !verified_kernel::merchant_scope_admissible(
        credential.is_some(),
        credential == resolved.as_deref(),
    ) {
        return Err(match credential {
            None => Error(
                StatusCode::UNAUTHORIZED,
                "Merchant credential required".into(),
            ),
            Some(_) => Error(
                StatusCode::FORBIDDEN,
                "Workspace does not match authenticated context".into(),
            ),
        });
    }
    Ok(resolved.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::{RequestContext, StatusCode, allowed, merchant, permit, validate_permissions};
    use serde_json::json;

    fn request(role: &str, permissions: Option<&str>) -> RequestContext {
        let mut h = RequestContext::new();
        h.insert("x-rac-role", role.parse().unwrap());
        if let Some(permissions) = permissions {
            h.insert("x-rac-permissions", permissions.parse().unwrap());
        }
        h
    }

    #[test]
    fn role_matrix() {
        for role in ["owner", "admin", "editor", "viewer"] {
            let h = request(role, None);
            assert!(permit(&h, "read").is_ok());
            assert_eq!(permit(&h, "catalog").is_ok(), role != "viewer");
            assert_eq!(
                permit(&h, "users").is_ok(),
                role == "owner" || role == "admin"
            );
            assert!(permit(&h, "unknown").is_err());
        }
        assert!(permit(&RequestContext::new(), "read").is_err());
    }

    #[test]
    fn a_request_without_identity_is_unauthenticated_and_a_missing_right_is_forbidden() {
        assert_eq!(
            permit(&RequestContext::new(), "catalog.read")
                .unwrap_err()
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            permit(&request("viewer", None), "catalog.write")
                .unwrap_err()
                .0,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn an_explicit_list_replaces_the_role_default_except_for_the_owner() {
        let narrowed = request("admin", Some(r#"["orders.read"]"#));
        assert!(allowed(&narrowed, "orders.read"));
        assert!(!allowed(&narrowed, "settings.write"));
        assert!(allowed(&request("owner", Some("[]")), "settings.write"));
        let widened = request("viewer", Some(r#"["catalog.write"]"#));
        assert!(allowed(&widened, "catalog.write"));
        // A broad right granted before the narrower scopes existed keeps covering them.
        assert!(allowed(&widened, "knowledge.write"));
        assert!(!allowed(&widened, "not.a.scope"));
        // An unreadable list grants nothing.
        assert!(!allowed(&request("admin", Some("not json")), "orders.read"));
    }

    #[test]
    fn nobody_delegates_a_right_they_lack_or_an_unknown_one() {
        let editor = request("editor", None);
        assert!(validate_permissions(&editor, &json!(["catalog.write"])).is_ok());
        assert_eq!(
            validate_permissions(&editor, &json!(["team.manage"]))
                .unwrap_err()
                .0,
            StatusCode::FORBIDDEN
        );
        assert!(validate_permissions(&editor, &json!(["made.up"])).is_err());
        assert!(validate_permissions(&editor, &json!("catalog.write")).is_err());
        assert!(
            validate_permissions(&editor, &json!(null))
                .unwrap()
                .is_null()
        );
    }

    #[test]
    fn a_credential_of_one_shop_does_not_act_for_another() {
        let mut h = request("admin", None);
        h.insert("x-rac-tenant", "shop-one".parse().unwrap());
        h.insert("x-tenant", "shop-two".parse().unwrap());
        assert_eq!(merchant(&h).unwrap_err().0, StatusCode::FORBIDDEN);
        h.insert("x-tenant", "shop-one".parse().unwrap());
        assert_eq!(merchant(&h).unwrap(), "shop-one");
    }

    #[test]
    fn a_merchant_request_needs_a_shop_credential() {
        assert_eq!(
            merchant(&request("admin", None)).unwrap_err().0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            merchant(&RequestContext::new()).unwrap_err().0,
            StatusCode::UNAUTHORIZED
        );
    }
}
