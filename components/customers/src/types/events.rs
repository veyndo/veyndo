//! The outbox events the customers component publishes: typed payloads behind `contracts/events/`.
use veyndo_core::event;

event! {
    /// A customer address was saved (only when the caller announces it).
    #[event(kinds = ["customer.address_changed"], version = 1)]
    pub struct AddressChanged<'a> {
        email: &'a str => "email",
        address_id: &'a str => "addressId",
    }
}

event! {
    /// A customer address was deleted.
    #[event(kinds = ["customer.address_deleted"], version = 1)]
    pub struct AddressDeleted<'a> {
        email: &'a str => "email",
        address_id: &'a str => "addressId",
    }
}

event! {
    /// A merchant created a customer (`customer.created`) or imported one from a migration source (`migration.record.imported`).
    #[event(kinds = ["customer.created", "migration.record.imported"], version = 1)]
    pub struct CustomerCreated<'a> {
        customer_id: &'a str => "customerId",
        actor: Option<&'a str> => "actor",
        source: &'a str => "source",
    }
}

event! {
    /// A merchant updated a customer's profile, group or activity.
    #[event(kinds = ["customer.updated"], version = 1)]
    pub struct CustomerUpdated<'a> {
        actor: Option<&'a str> => "actor",
        email: &'a str => "email",
        revision: i64 => "revision",
        active: bool => "active",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        AddressChanged::SPEC,
        AddressDeleted::SPEC,
        CustomerCreated::SPEC,
        CustomerUpdated::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let actor: Option<&str> = Some("merchant@example.test");
        let checks = [
            (
                AddressChanged {
                    email: "e",
                    address_id: "a",
                }
                .payload(),
                json!({"email":"e","addressId":"a"}),
            ),
            (
                AddressDeleted {
                    email: "e",
                    address_id: "a",
                }
                .payload(),
                json!({"email":"e","addressId":"a"}),
            ),
            (
                CustomerCreated {
                    customer_id: "c",
                    actor: None,
                    source: "merchant",
                }
                .payload(),
                json!({"customerId":"c","actor":None::<&str>,"source":"merchant"}),
            ),
            (
                CustomerUpdated {
                    actor,
                    email: "e",
                    revision: 3,
                    active: false,
                }
                .payload(),
                json!({"actor":actor,"email":"e","revision":3,"active":false}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
