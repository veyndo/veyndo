//! Which profile fields are shown as top-level contact fields of a customer.
use serde_json::Value;

/// Copies the contact fields of the stored profile to the top level.
/// Identity, status and price group never come from the profile.
pub fn project_contacts(v: &mut Value) {
    for key in [
        "name",
        "firstName",
        "lastName",
        "salutationId",
        "title",
        "phoneNumber",
        "birthday",
        "vatIds",
        "defaultPaymentMethodId",
    ] {
        if let Some(value) = v["profile"].get(key).cloned() {
            v[key] = value;
        }
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
