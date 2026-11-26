//! Which profile fields are shown as top-level contact fields of a customer.
use serde::Deserialize;
use serde_json::{Map, Value};
use veyndo_core::document::{lenient, merge, view};

/// The profile fields that are shown as top-level contact fields.
const CONTACT_FIELDS: [&str; 9] = [
    "name",
    "firstName",
    "lastName",
    "salutationId",
    "title",
    "phoneNumber",
    "birthday",
    "vatIds",
    "defaultPaymentMethodId",
];

/// The stored profile of a customer value.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Stored {
    #[serde(deserialize_with = "lenient")]
    profile: Map<String, Value>,
}

/// Copies the contact fields of the stored profile to the top level.
/// Identity, status and price group never come from the profile.
pub fn project_contacts(v: &mut Value) {
    let stored: Stored = view(v);
    let contacts: Map<String, Value> = CONTACT_FIELDS
        .iter()
        .filter_map(|key| {
            stored
                .profile
                .get(*key)
                .map(|value| ((*key).to_owned(), value.clone()))
        })
        .collect();
    if !contacts.is_empty() {
        // A value with a profile object is itself an object, so the merge cannot fail.
        let _ = merge(v, Value::Object(contacts));
    }
}

#[cfg(test)]
mod tests {
    use super::project_contacts;
    use serde_json::json;

    #[test]
    fn legacy_profile_cannot_replace_account_authority() {
        let mut v = json!({"id":"own-id","email":"own@example.test","active":true,"profile":{"id":"foreign-id","email":"foreign@example.test","active":false,"customerGroup":"business","password_hash":"secret","firstName":"Alex"}});
        project_contacts(&mut v);
        assert_eq!(v["id"], "own-id");
        assert_eq!(v["email"], "own@example.test");
        assert_eq!(v["active"], true);
        assert_eq!(v["firstName"], "Alex");
        assert!(v.get("password_hash").is_none());
        assert!(v.get("customerGroup").is_none());
    }
}
