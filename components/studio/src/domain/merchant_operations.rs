//! The merchant operation catalogue shared by HTTP and MCP: the native right of each operation, compound manual-order admission and the operations that touch personal data.
use veyndo_core::RequestContext;
use veyndo_identity::allowed;

/// The native right each merchant operation requires.
pub fn permission(name: &str) -> Option<&'static str> {
    Some(match name {
        "merchant.import.record" | "merchant.import.inspect" => "apps.manage",
        "merchant.grid.plan" => "assistant.use",
        "merchant.company" => "settings.read",
        "merchant.company.save" => "settings.write",
        "merchant.workflow" => "orders.read",
        "merchant.workflow.save" => "settings.write",
        "merchant.customer.lookup" | "merchant.customers" | "merchant.customer" => "customers.read",
        "merchant.customer.creation"
        | "merchant.customer.create"
        | "merchant.customer.save"
        | "merchant.customer.address.save"
        | "merchant.customer.address.delete" => "customers.write",
        "merchant.customer.addresses" => "customers.read",
        "merchant.orders" | "merchant.order" => "orders.read",
        "merchant.order.context"
        | "merchant.order.review"
        | "merchant.order.complete"
        | "merchant.order.transition"
        | "merchant.order.note"
        | "merchant.delivery.split" => "orders.write",
        "merchant.receipts" => "documents.read",
        "merchant.receipt.create" => "documents.create",
        "merchant.payment" | "merchant.payment.onboarding" => "payments.manage",
        "merchant.payment.providers"
        | "merchant.payment.details"
        | "merchant.payment.instructions" => "payments.read",
        "merchant.products"
        | "merchant.categories"
        | "merchant.product.assets"
        | "merchant.product.content" => "catalog.read",
        "merchant.product.create"
        | "merchant.category.create"
        | "merchant.category.save"
        | "merchant.asset.publish"
        | "merchant.product.save" => "catalog.write",
        _ => return None,
    })
}
/// Compound admission for manual ordering is identical in discovery and invocation.
pub fn creation_visible(h: &RequestContext, name: &str) -> bool {
    if name == "merchant.payment.instructions" {
        return allowed(h, "orders.read");
    }
    !matches!(
        name,
        "merchant.order.context" | "merchant.order.review" | "merchant.order.complete"
    ) || [
        "orders.write",
        "customers.read",
        "customers.pii",
        "catalog.read",
    ]
    .iter()
    .all(|s| allowed(h, s))
}
/// Operations that read or write customer personal data additionally require `customers.pii`.
pub fn requires_pii(name: &str) -> bool {
    name.starts_with("merchant.customer.")
        || matches!(
            name,
            "merchant.payment.instructions"
                | "merchant.customers"
                | "merchant.customer"
                | "merchant.orders"
                | "merchant.order"
                | "merchant.order.transition"
                | "merchant.order.note"
                | "merchant.order.context"
                | "merchant.order.review"
                | "merchant.order.complete"
                | "merchant.delivery.split"
                | "merchant.receipts"
                | "merchant.receipt.create"
        )
}

#[cfg(test)]
mod tests {
    use super::{creation_visible, permission, requires_pii};
    use veyndo_core::RequestContext;

    #[test]
    fn every_operation_names_its_native_right() {
        assert_eq!(permission("merchant.order.complete"), Some("orders.write"));
        assert_eq!(permission("merchant.payment"), Some("payments.manage"));
        assert_eq!(permission("merchant.unknown"), None);
    }

    #[test]
    fn personal_data_operations_are_marked() {
        assert!(requires_pii("merchant.customer.save"));
        assert!(requires_pii("merchant.order.review"));
        assert!(!requires_pii("merchant.products"));
    }

    #[test]
    fn manual_orders_need_the_full_compound_right_to_be_visible() {
        let h = RequestContext::default();
        assert!(!creation_visible(&h, "merchant.order.review"));
        assert!(creation_visible(&h, "merchant.products"));
    }
}
