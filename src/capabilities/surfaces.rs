//! Protocol surfaces of the operations graph: the native operations each UCP capability exposes; discovery and the buyer negotiation allowlist are generated from it.
use crate::ucp::{
    UCP_B2B, UCP_B2B_SCHEMA, UCP_B2B_VERSION, UCP_CART, UCP_CHECKOUT, UCP_CONSENT, UCP_VERSION,
};

/// One advertised UCP capability and the native operations that implement it.
pub(crate) struct UcpDeclaration {
    pub(crate) id: &'static str,
    pub(crate) version: &'static str,
    pub(crate) spec: &'static str,
    pub(crate) schema: &'static str,
    pub(crate) extends: &'static [&'static str],
    /// Native operations whose facts are the facts of this capability; all are shopper operations.
    pub(crate) operations: &'static [&'static str],
}

pub(crate) const UCP: &[UcpDeclaration] = &[
    UcpDeclaration {
        id: UCP_CHECKOUT,
        version: UCP_VERSION,
        spec: "https://ucp.dev/2026-08-25/specification/shopping/checkout/",
        schema: "https://ucp.dev/2026-08-25/schemas/shopping/checkout.json",
        extends: &[],
        operations: &[
            "checkout.options",
            "checkout.select",
            "cart.quote",
            "checkout.complete",
        ],
    },
    UcpDeclaration {
        id: UCP_B2B,
        version: UCP_B2B_VERSION,
        spec: "https://www.veyndo.ai/docs/b2b-ucp-negotiation.html",
        schema: UCP_B2B_SCHEMA,
        extends: &[UCP_CHECKOUT],
        operations: &[
            "b2b.contexts",
            "b2b.select",
            "b2b.quote.current",
            "b2b.quote.get",
            "b2b.quote.request",
            "b2b.quote.message",
            "b2b.quote.reject",
            "b2b.quote.accept",
            "b2b.quote.release",
            "b2b.approval.request",
        ],
    },
    UcpDeclaration {
        id: UCP_CART,
        version: UCP_VERSION,
        spec: "https://ucp.dev/2026-08-25/specification/shopping/cart/",
        schema: "https://ucp.dev/2026-08-25/schemas/shopping/cart.json",
        extends: &[],
        operations: &["cart.create", "cart.replace", "cart.quote"],
    },
    UcpDeclaration {
        id: UCP_CONSENT,
        version: UCP_VERSION,
        spec: "https://ucp.dev/2026-08-25/specification/buyer-consent/",
        schema: "https://ucp.dev/2026-08-25/schemas/shopping/buyer_consent.json",
        extends: &[UCP_CHECKOUT, UCP_CART],
        operations: &["privacy.policy", "privacy.consent"],
    },
];

/// The declaration of one advertised capability.
pub(crate) fn ucp(id: &str) -> Option<&'static UcpDeclaration> {
    UCP.iter().find(|declaration| declaration.id == id)
}

#[cfg(test)]
mod tests {
    use super::UCP;
    use crate::capabilities::registry::find;
    use veyndo_core::operation_facts::Audience;

    #[test]
    fn protocol_operations_exist_and_never_reach_merchant_authority() {
        for declaration in UCP {
            assert!(!declaration.operations.is_empty(), "{}", declaration.id);
            for name in declaration.operations {
                let operation = find(name).unwrap_or_else(|| panic!("{name} is no operation"));
                assert_eq!(operation.facts.audience, Audience::Shopper, "{name}");
            }
            for parent in declaration.extends {
                assert!(super::ucp(parent).is_some(), "{parent}");
            }
        }
    }
}
