//! The outbox events the b2b component publishes: typed payloads behind `contracts/events/`.
use serde_json::Value;
use veyndo_core::event;

event! {
    /// A company or purchasing unit was saved; `kind` names the table (`company` or `unit`).
    #[event(kinds = ["b2b.organization_changed"], version = 1)]
    pub struct OrganizationChanged<'a> {
        id: &'a str => "id",
        kind: &'a str => "kind",
        revision: i64 => "revision",
    }
}

event! {
    /// The grant of one customer (employee) in a purchasing unit was saved.
    #[event(kinds = ["b2b.employee_changed"], version = 1)]
    pub struct EmployeeChanged<'a> {
        unit_id: &'a str => "unitId",
        customer_id: &'a str => "customerId",
        revision: i64 => "revision",
    }
}

event! {
    /// The private catalogue of a purchasing unit was saved.
    #[event(kinds = ["b2b.catalog_changed"], version = 1)]
    pub struct CatalogChanged<'a> {
        unit_id: &'a str => "unitId",
        revision: i64 => "revision",
    }
}

event! {
    /// A shared list of a purchasing unit was saved or deleted (`operation`: the list operation).
    #[event(kinds = ["b2b.list_changed"], version = 1)]
    pub struct ListChanged<'a> {
        list_id: &'a str => "listId",
        unit_id: &'a str => "unitId",
        operation: &'a str => "operation",
        revision: i64 => "revision",
    }
}

event! {
    /// A buyer created a requisition list.
    #[event(kinds = ["b2b.list_changed"], version = 1)]
    pub struct RequisitionCreated<'a> {
        id: &'a str => "id",
        unit_id: &'a str => "unitId",
        revision: i64 => "revision",
    }
}

event! {
    /// A purchase exceeded the buyer's limits and waits for an approver.
    #[event(kinds = ["b2b.approval_requested"], version = 1)]
    pub struct ApprovalRequested<'a> {
        approval_id: &'a str => "approvalId",
        unit_id: &'a str => "unitId",
    }
}

event! {
    /// An approver approved or rejected a purchase.
    #[event(kinds = ["b2b.approval_decided"], version = 1)]
    pub struct ApprovalDecided<'a> {
        approval_id: &'a str => "approvalId",
        unit_id: &'a str => "unitId",
        state: &'a str => "state",
    }
}

event! {
    /// A company purchase was placed against its budget; `totalMinor` is in minor units of `currency`.
    #[event(kinds = ["b2b.purchase_completed"], version = 1)]
    pub struct PurchaseCompleted<'a> {
        order_id: &'a str => "orderId",
        company_id: &'a str => "companyId",
        unit_id: &'a str => "unitId",
        currency: &'a str => "currency",
        total_minor: i64 => "totalMinor",
        approval_id: Option<&'a str> => "approvalId",
    }
}

event! {
    /// A buyer requested a quote for the reviewed cart.
    #[event(kinds = ["b2b.quote_requested"], version = 1)]
    pub struct QuoteRequested<'a> {
        quote_id: &'a str => "quoteId",
        unit_id: &'a str => "unitId",
    }
}

event! {
    /// A quote moved between states; `lifecycle` is the app lifecycle transition record or `null`.
    #[event(kinds = ["b2b.quote_changed"], version = 1)]
    pub struct QuoteChanged<'a> {
        quote_id: &'a str => "quoteId",
        state: &'a str => "state",
        from: &'a str => "from",
        revision: i64 => "revision",
        lifecycle: Value => "lifecycle",
    }
}

event! {
    /// A buyer selected an offered quote for the cart.
    #[event(kinds = ["b2b.quote_selected"], version = 1)]
    pub struct QuoteSelected<'a> {
        cart_id: &'a str => "cartId",
        quote_id: Option<&'a str> => "quoteId",
        revision: i64 => "revision",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        OrganizationChanged::SPEC,
        EmployeeChanged::SPEC,
        CatalogChanged::SPEC,
        ListChanged::SPEC,
        RequisitionCreated::SPEC,
        ApprovalRequested::SPEC,
        ApprovalDecided::SPEC,
        PurchaseCompleted::SPEC,
        QuoteRequested::SPEC,
        QuoteChanged::SPEC,
        QuoteSelected::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let quote = Some(String::from("q"));
        let checks = [
            (
                OrganizationChanged {
                    id: "u",
                    kind: "unit",
                    revision: 2,
                }
                .payload(),
                json!({"id":"u","kind":"unit","revision":1+1}),
            ),
            (
                EmployeeChanged {
                    unit_id: "u",
                    customer_id: "c",
                    revision: 2,
                }
                .payload(),
                json!({"unitId":"u","customerId":"c","revision":2}),
            ),
            (
                CatalogChanged {
                    unit_id: "u",
                    revision: 3,
                }
                .payload(),
                json!({"unitId":"u","revision":3}),
            ),
            (
                ListChanged {
                    list_id: "l",
                    unit_id: "u",
                    operation: "list.delete",
                    revision: 2,
                }
                .payload(),
                json!({"listId":"l","unitId":"u","operation":"list.delete","revision":2}),
            ),
            (
                RequisitionCreated {
                    id: "l",
                    unit_id: "u",
                    revision: 1,
                }
                .payload(),
                json!({"id":"l","unitId":"u","revision":1}),
            ),
            (
                ApprovalRequested {
                    approval_id: "a",
                    unit_id: "u",
                }
                .payload(),
                json!({"approvalId":"a","unitId":"u"}),
            ),
            (
                ApprovalDecided {
                    approval_id: "a",
                    unit_id: "u",
                    state: "approved",
                }
                .payload(),
                json!({"approvalId":"a","unitId":"u","state":"approved"}),
            ),
            (
                PurchaseCompleted {
                    order_id: "o",
                    company_id: "c",
                    unit_id: "u",
                    currency: "EUR",
                    total_minor: 1250,
                    approval_id: None,
                }
                .payload(),
                json!({"orderId":"o","companyId":"c","unitId":"u","currency":"EUR","totalMinor":1250i64,"approvalId":None::<String>}),
            ),
            (
                QuoteRequested {
                    quote_id: "q",
                    unit_id: "u",
                }
                .payload(),
                json!({"quoteId":"q","unitId":"u"}),
            ),
            (
                QuoteChanged {
                    quote_id: "q",
                    state: "offered",
                    from: "requested",
                    revision: 2,
                    lifecycle: Value::Null,
                }
                .payload(),
                json!({"quoteId":"q","state":"offered","from":"requested","revision":2,"lifecycle":Value::Null}),
            ),
            (
                QuoteSelected {
                    cart_id: "c",
                    quote_id: quote.as_deref(),
                    revision: 4,
                }
                .payload(),
                json!({"cartId":"c","quoteId":quote.as_ref(),"revision":4}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
