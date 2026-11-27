//! The outbox events the identity component publishes: typed payloads behind `contracts/events/`.
use serde_json::Value;
use veyndo_core::event;

event! {
    /// A merchant account recovered its password; older sessions and handoffs were revoked.
    #[event(kinds = ["account.security.updated"], version = 1)]
    pub struct PasswordChanged<'a> {
        user_id: &'a str => "userId",
        action: &'a str => "action",
        credential_revision: i64 => "credentialRevision",
    }
}

event! {
    /// A merchant account enabled or disabled its second factor (`action`: `mfa.enabled` / `mfa.disabled`).
    #[event(kinds = ["account.security.updated"], version = 1)]
    pub struct MfaChanged<'a> {
        user_id: &'a str => "userId",
        action: &'a str => "action",
        revision: i64 => "revision",
    }
}

event! {
    /// A team membership changed role, activity or permissions; `permissions` is the explicit list or `null` for the role's grants.
    #[event(kinds = ["membership.updated"], version = 1)]
    pub struct MembershipUpdated<'a> {
        actor: Option<&'a str> => "actor",
        subject: &'a str => "subject",
        role: &'a str => "role",
        active: bool => "active",
        permissions: &'a Value => "permissions",
        previous_role: &'a str => "previousRole",
        previous_active: bool => "previousActive",
    }
}

event! {
    /// A custom team role was saved or deleted.
    #[event(kinds = ["team.role_changed"], version = 1)]
    pub struct RoleChanged<'a> {
        actor: Option<&'a str> => "actor",
        id: &'a str => "id",
        revision: i64 => "revision",
        deleted: bool => "deleted",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        PasswordChanged::SPEC,
        MfaChanged::SPEC,
        MembershipUpdated::SPEC,
        RoleChanged::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let permissions = json!(["orders.read"]);
        let checks = [
            (
                PasswordChanged {
                    user_id: "u",
                    action: "password.changed",
                    credential_revision: 3,
                }
                .payload(),
                json!({"userId":"u","action":"password.changed","credentialRevision":2+1}),
            ),
            (
                MfaChanged {
                    user_id: "u",
                    action: "mfa.enabled",
                    revision: 2,
                }
                .payload(),
                json!({"userId":"u","action":"mfa.enabled","revision":2}),
            ),
            (
                MembershipUpdated {
                    actor: Some("o"),
                    subject: "u",
                    role: "staff",
                    active: true,
                    permissions: &permissions,
                    previous_role: "admin",
                    previous_active: true,
                }
                .payload(),
                json!({"actor":Some("o"),"subject":"u","role":"staff","active":true,"permissions":permissions,"previousRole":"admin","previousActive":true}),
            ),
            (
                RoleChanged {
                    actor: None,
                    id: "r",
                    revision: 2,
                    deleted: false,
                }
                .payload(),
                json!({"actor":None::<&str>,"id":"r","revision":2,"deleted":false}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
