//! Which entities keep a history, the rights their versions need, and what a restoration request must state.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad};
use veyndo_identity::domain::permissions::allowed;

/// The entities whose changes are recorded as versions.
pub const ENTITIES: &[&str] = &[
    "role",
    "product",
    "category",
    "customer",
    "order",
    "settings",
    "company",
    "companyChannel",
    "checkoutChannel",
    "rule",
    "flow",
    "promotion",
    "channel",
    "source",
    "b2bCompany",
    "b2bUnit",
    "b2bQuote",
    "b2bList",
    "b2bApproval",
];

/// The history operations offered over HTTP and MCP alike.
pub const OPERATIONS: [&str; 3] = [
    "merchant.history",
    "merchant.history.version",
    "merchant.history.restore",
];

/// Whether the versions of an entity contain personal data.
pub fn requires_pii(entity: &str) -> bool {
    entity.starts_with("b2b") || ["customer", "order"].contains(&entity)
}

/// The right to read the versions of an entity and, where a version may be restored, the right to do so.
pub fn rights(entity: &str) -> Result<(&'static str, Option<&'static str>)> {
    Ok(match entity {
        "role" => ("team.manage", Some("team.manage")),
        "product" | "category" => ("catalog.read", Some("catalog.write")),
        "source" => ("knowledge.read", Some("knowledge.write")),
        "customer" => ("customers.read", Some("customers.write")),
        "order" => ("orders.read", None),
        "b2bCompany" | "b2bUnit" => ("b2b.read", Some("b2b.manage")),
        "b2bQuote" | "b2bList" | "b2bApproval" => ("b2b.read", None),
        "channel" => ("channels.read", Some("channels.manage")),
        "rule" | "flow" | "promotion" => ("automation.read", Some("automation.manage")),
        "settings" | "checkoutChannel" | "company" | "companyChannel" => {
            ("settings.read", Some("settings.write"))
        }
        _ => return Err(bad("Unsupported history entity")),
    })
}

/// Whether a request may read the versions of an entity, or restore one of them.
pub fn accessible(h: &RequestContext, entity: &str, restore: bool) -> bool {
    rights(entity).is_ok_and(|(read, write)| {
        allowed(h, read)
            && (!requires_pii(entity) || allowed(h, "customers.pii"))
            && (!restore || write.is_some_and(|scope| allowed(h, scope)))
    })
}

/// Whether a history operation is offered to a request at all: some entity must be within its rights.
pub fn visible(h: &RequestContext, name: &str) -> bool {
    OPERATIONS.contains(&name)
        && ENTITIES
            .iter()
            .any(|entity| accessible(h, entity, name == "merchant.history.restore"))
}

/// The input schema of a history operation; `None` for any other operation.
pub fn schema(name: &str) -> Option<Value> {
    if !OPERATIONS.contains(&name) {
        return None;
    }
    let mut props = json!({"entity":{"type":"string","enum":ENTITIES},"id":{"type":"string","minLength":1,"maxLength":254}});
    let mut required = vec!["entity", "id"];
    if name == "merchant.history" {
        props["before"] = json!({"type":"integer"});
    } else {
        props["version"] = json!({"type":"integer","minimum":1});
        required.push("version");
    }
    if name == "merchant.history.restore" {
        props["approve"] = json!({"type":"boolean","const":true});
        props["revision"] = json!({"type":"integer","minimum":0});
        props["side"] = json!({"type":"string","enum":["before","after"]});
        required.extend(["approve", "revision", "side"]);
    }
    Some(
        json!({"type":"object","properties":props,"required":required,"additionalProperties":false}),
    )
}

/// The current revision and the side of a version an approved restoration names.
pub fn restoration(v: &Value) -> Result<(i64, &'static str)> {
    let input: Restoration = view(v);
    if input.approve != true {
        return Err(bad("Explicit restoration approval required"));
    }
    let revision = input.revision.ok_or(bad("Current revision required"))?;
    let side = match input.side.as_deref() {
        Some("before") => "before",
        Some("after") => "after",
        _ => return Err(bad("Version side required")),
    };
    Ok((revision, side))
}

/// The fields of an approved restoration request.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Restoration {
    approve: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    side: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{ENTITIES, accessible, requires_pii, restoration, rights, schema, visible};
    use serde_json::json;
    use veyndo_core::RequestContext;

    fn member(role: &'static str) -> RequestContext {
        let mut h = RequestContext::new();
        h.insert("x-rac-role", role.parse().unwrap());
        h
    }

    #[test]
    fn every_recorded_entity_has_rights_and_orders_are_never_rewound() {
        for entity in ENTITIES {
            assert!(rights(entity).is_ok(), "{entity}");
        }
        assert!(rights("payment").is_err());
        assert_eq!(rights("order").unwrap(), ("orders.read", None));
        assert_eq!(
            rights("flow").unwrap(),
            ("automation.read", Some("automation.manage"))
        );
        assert!(requires_pii("customer"));
        assert!(requires_pii("order"));
        assert!(requires_pii("b2bQuote"));
        assert!(!requires_pii("product"));
    }

    #[test]
    fn access_follows_the_entity_rights_and_personal_data_needs_its_own() {
        let viewer = member("viewer");
        assert!(accessible(&viewer, "product", false));
        assert!(!accessible(&viewer, "product", true));
        assert!(!accessible(&viewer, "customer", false));
        let editor = member("editor");
        assert!(accessible(&editor, "product", true));
        assert!(!accessible(&editor, "settings", true));
        let admin = member("admin");
        assert!(accessible(&admin, "customer", true));
        assert!(accessible(&admin, "order", false));
        assert!(!accessible(&admin, "order", true));
        assert!(!accessible(&admin, "payment", false));
        assert!(!accessible(&RequestContext::new(), "product", false));
    }

    #[test]
    fn operations_are_offered_only_within_some_entity_right() {
        let viewer = member("viewer");
        assert!(visible(&viewer, "merchant.history"));
        assert!(visible(&viewer, "merchant.history.version"));
        assert!(!visible(&viewer, "merchant.history.restore"));
        assert!(visible(&member("editor"), "merchant.history.restore"));
        assert!(!visible(&viewer, "merchant.orders"));
        assert!(!visible(&RequestContext::new(), "merchant.history"));
    }

    #[test]
    fn schemas_state_exactly_what_each_operation_needs() {
        assert!(schema("merchant.orders").is_none());
        let list = schema("merchant.history").unwrap();
        assert_eq!(list["required"], json!(["entity", "id"]));
        assert!(list["properties"]["before"].is_object());
        let version = schema("merchant.history.version").unwrap();
        assert_eq!(version["required"], json!(["entity", "id", "version"]));
        let restore = schema("merchant.history.restore").unwrap();
        assert_eq!(
            restore["required"],
            json!(["entity", "id", "version", "approve", "revision", "side"])
        );
        assert_eq!(restore["properties"]["approve"]["const"], true);
        assert_eq!(restore["additionalProperties"], false);
    }

    #[test]
    fn a_restoration_needs_approval_the_current_revision_and_a_side() {
        let request = json!({"approve":true,"revision":4,"side":"before"});
        assert_eq!(restoration(&request).unwrap(), (4, "before"));
        assert!(restoration(&json!({"revision":4,"side":"before"})).is_err());
        assert!(restoration(&json!({"approve":true,"side":"after"})).is_err());
        assert!(restoration(&json!({"approve":true,"revision":4,"side":"both"})).is_err());
    }
}
