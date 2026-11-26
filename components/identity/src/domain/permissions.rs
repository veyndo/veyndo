//! The permission vocabulary and the decision whether a request holds one. Owners retain control; delegates cannot grant rights they lack.
use serde_json::Value;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_kernel::verified_kernel;

pub const SCOPES: &[&str] = &[
    "overview.read",
    "assistant.use",
    "automation.read",
    "automation.manage",
    "channels.read",
    "channels.manage",
    "developer.read",
    "developer.manage",
    "environments.read",
    "environments.manage",
    "knowledge.write",
    "ai.manage",
    "apps.read",
    "pos.use",
    "b2b.read",
    "b2b.manage",
    "catalog.read",
    "catalog.write",
    "orders.read",
    "orders.write",
    "customers.read",
    "customers.pii",
    "customers.write",
    "documents.read",
    "documents.create",
    "payments.read",
    "payments.manage",
    "team.manage",
    "settings.read",
    "settings.write",
    "apps.manage",
    "knowledge.read",
];
pub fn scope(kind: &str) -> &str {
    match kind {
        "catalog" => "catalog.write",
        "users" => "team.manage",
        "operations" => "orders.write",
        "settings" => "settings.write",
        "extension" => "apps.manage",
        x => x,
    }
}
pub fn allowed(h: &RequestContext, kind: &str) -> bool {
    let role = header(h, "x-rac-role").unwrap_or("");
    if kind == "read" {
        return !role.is_empty();
    }
    let requested = scope(kind);
    let raw = header(h, "x-rac-permissions");
    let explicit_grant = raw.is_some_and(|raw| {
        serde_json::from_str::<Vec<String>>(raw)
            .is_ok_and(|v| v.iter().any(|x| x == requested) || compatibility_grant(&v, requested))
    });
    let default_grant = match requested {
        "catalog.read" | "orders.read" | "payments.read" | "settings.read" | "knowledge.read"
        | "overview.read" | "assistant.use" | "automation.read" | "channels.read"
        | "environments.read" | "apps.read" => !role.is_empty(),
        "catalog.write" | "knowledge.write" => ["admin", "editor"].contains(&role),
        x if SCOPES.contains(&x) => role == "admin",
        _ => false,
    };
    verified_kernel::scope_admissible(
        !role.is_empty(),
        SCOPES.contains(&requested),
        role == "owner",
        raw.is_some(),
        explicit_grant,
        default_grant,
    )
}
/// Existing broad settings rights remain broad; new domain scopes permit narrower delegation.
fn compatibility_grant(values: &[String], requested: &str) -> bool {
    let has = |s: &str| values.iter().any(|v| v == s);
    match requested {
        "channels.read" | "automation.read" => has("settings.read"),
        "channels.manage" | "automation.manage" | "ai.manage" => has("settings.write"),
        "developer.read" | "developer.manage" => has("apps.manage") && has("team.manage"),
        "environments.read" | "environments.manage" => has("team.manage"),
        "knowledge.write" => has("catalog.write"),
        "apps.read" => has("apps.manage"),
        "overview.read" => has("orders.read"),
        _ => false,
    }
}
pub fn validate_permissions(h: &RequestContext, v: &Value) -> Result<Value> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    let values = v
        .as_array()
        .filter(|x| x.len() <= SCOPES.len())
        .ok_or(bad("Invalid permission list"))?;
    for x in values {
        let x = x.as_str().ok_or(bad("Invalid permission"))?;
        if !SCOPES.contains(&x) {
            return Err(bad("Unknown permission"));
        }
        if !allowed(h, x) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Cannot delegate a permission you do not hold".into(),
            ));
        }
    }
    Ok(v.clone())
}
/// A right a budgeted planning agent may hold: reading, the assistant and personal data, never a business write.
pub fn planning_scope(scope: &str) -> bool {
    scope.ends_with(".read") || ["assistant.use", "customers.pii"].contains(&scope)
}

#[cfg(test)]
mod tests {
    use super::{allowed, planning_scope, scope, validate_permissions};
    use serde_json::{Value, json};
    use veyndo_core::{RequestContext, StatusCode};

    fn member(role: &'static str, permissions: Option<&'static str>) -> RequestContext {
        let mut h = RequestContext::new();
        h.insert("x-rac-role", role.parse().unwrap());
        if let Some(permissions) = permissions {
            h.insert("x-rac-permissions", permissions.parse().unwrap());
        }
        h
    }

    #[test]
    fn earlier_area_names_map_to_their_scope() {
        assert_eq!(scope("catalog"), "catalog.write");
        assert_eq!(scope("users"), "team.manage");
        assert_eq!(scope("operations"), "orders.write");
        assert_eq!(scope("settings"), "settings.write");
        assert_eq!(scope("extension"), "apps.manage");
        assert_eq!(scope("orders.read"), "orders.read");
    }

    #[test]
    fn roles_hold_their_default_rights_and_owners_every_known_one() {
        let anonymous = RequestContext::new();
        assert!(!allowed(&anonymous, "read"));
        assert!(!allowed(&anonymous, "catalog.read"));
        let viewer = member("viewer", None);
        assert!(allowed(&viewer, "read"));
        assert!(allowed(&viewer, "catalog.read"));
        assert!(!allowed(&viewer, "catalog.write"));
        assert!(!allowed(&viewer, "customers.pii"));
        let editor = member("editor", None);
        assert!(allowed(&editor, "catalog"));
        assert!(allowed(&editor, "knowledge.write"));
        assert!(!allowed(&editor, "orders.write"));
        let admin = member("admin", None);
        assert!(allowed(&admin, "users"));
        assert!(allowed(&admin, "customers.pii"));
        let owner = member("owner", None);
        assert!(allowed(&owner, "payments.manage"));
        for h in [&owner, &admin] {
            assert!(!allowed(h, "orders.delete-everything"));
        }
    }

    #[test]
    fn an_explicit_list_replaces_the_role_defaults_and_keeps_broad_rights_broad() {
        let delegate = member(
            "viewer",
            Some(r#"["orders.write","settings.write","catalog.write"]"#),
        );
        assert!(allowed(&delegate, "orders.write"));
        assert!(allowed(&delegate, "channels.manage"));
        assert!(allowed(&delegate, "automation.manage"));
        assert!(allowed(&delegate, "knowledge.write"));
        assert!(!allowed(&delegate, "team.manage"));
        assert!(!allowed(&delegate, "catalog.read"));
        let broken = member("admin", Some("not json"));
        assert!(!allowed(&broken, "orders.write"));
    }

    #[test]
    fn nobody_delegates_a_right_they_do_not_hold() {
        let admin = member("admin", None);
        assert_eq!(
            validate_permissions(&admin, &Value::Null).ok(),
            Some(Value::Null)
        );
        let list = json!(["orders.write", "catalog.read"]);
        assert_eq!(validate_permissions(&admin, &list).ok(), Some(list));
        assert!(validate_permissions(&admin, &json!("orders.write")).is_err());
        assert!(validate_permissions(&admin, &json!([7])).is_err());
        assert!(validate_permissions(&admin, &json!(["orders.everything"])).is_err());
        let editor = member("editor", None);
        assert_eq!(
            validate_permissions(&editor, &json!(["team.manage"]))
                .err()
                .map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        assert!(validate_permissions(&editor, &json!(["catalog.write"])).is_ok());
    }

    #[test]
    fn planning_agents_read_and_never_write() {
        for scope in [
            "catalog.read",
            "orders.read",
            "assistant.use",
            "customers.pii",
        ] {
            assert!(planning_scope(scope), "{scope}");
        }
        for scope in [
            "catalog.write",
            "orders.write",
            "team.manage",
            "payments.manage",
        ] {
            assert!(!planning_scope(scope), "{scope}");
        }
    }
}
