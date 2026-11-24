//! What a flow sees of an outbox event: personal receipts and payment instructions removed, the order states and the event folded into one rule context.
use crate::types::flow::{knowledge_event, valid_app_event};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_flow_graph::catalog::EVENTS;

/// Whether an event kind can start a flow at all.
pub fn projected(kind: &str) -> bool {
    EVENTS.contains(&kind) || valid_app_event(kind)
}

/// Whether an event without an order still starts flows: app, knowledge and product events.
pub fn orderless(kind: &str) -> bool {
    valid_app_event(kind)
        || knowledge_event(kind)
        || ["product.created", "product.updated"].contains(&kind)
}

/// The event as generic automation sees it; customer receipts and payment instructions stay with their subscribed apps.
pub fn public_event(kind: &str, data: &Value) -> Value {
    let mut event = data.clone();
    if let Some(fields) = event.as_object_mut() {
        if kind.starts_with("consumer.") {
            fields.remove("receipt");
        }
        if kind == "payment.instructions.available" {
            fields.remove("paymentInstructions");
        }
    }
    event
}

/// The rule context of an event: the order's cart with the event, its kind and the order, payment and delivery states.
pub fn rule_context(kind: &str, event: &Value, order: &Value) -> Value {
    let order: ContextOrder = view(order);
    let mut context = order.cart;
    context["event"] = event.clone();
    context["eventKind"] = json!(kind);
    context["orderState"] = order.state;
    context["paymentState"] = order.payment.state;
    context["deliveryStates"] = json!(
        order
            .deliveries
            .map(|ds| ds
                .iter()
                .map(|d| view::<StateField>(d).state)
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    context
}

/// The order fields a flow's rule context is built from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ContextOrder {
    cart: Value,
    state: Value,
    #[serde(deserialize_with = "lenient")]
    payment: StateField,
    #[serde(deserialize_with = "lenient")]
    deliveries: Option<Vec<Value>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StateField {
    state: Value,
}

/// The frozen definition a matched flow job runs from.
pub fn job_definition(
    flow: &Value,
    kind: &str,
    event: &Value,
    order_id: &str,
    order: &Value,
    cart: &Value,
    context: &Value,
) -> Value {
    let order: FlowOrder = view(order);
    let total = order
        .cart
        .price
        .get("totalPrice")
        .cloned()
        .unwrap_or_default();
    json!({"flow":flow,"orderId":order_id,"orderNumber":order.order_number,"totalPrice":total,"event":kind,"transition":event,"eventContext":event,"cartData":cart,"ruleContext":context,"orderSnapshot":{"orderCustomer":order.order_customer,"currencyId":order.currency_id,"orderNumber":order.order_number,"state":order.state,"cart":{"price":order.cart.price}}})
}

/// The order snapshot a flow job carries.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FlowOrder {
    order_number: Value,
    order_customer: Value,
    currency_id: Value,
    state: Value,
    #[serde(deserialize_with = "lenient")]
    cart: FlowCart,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FlowCart {
    price: Value,
}

#[cfg(test)]
mod tests {
    use super::{job_definition, orderless, projected, public_event, rule_context};
    use serde_json::json;

    #[test]
    fn only_catalogued_and_app_events_start_flows() {
        assert!(projected("order.placed"));
        assert!(projected("app.reviews.submitted"));
        assert!(!projected("app.reviews"));
        assert!(!projected("unknown.event"));
        assert!(orderless("app.reviews.submitted"));
        assert!(orderless("product.updated"));
        assert!(!orderless("order.placed"));
    }

    #[test]
    fn personal_receipts_and_payment_instructions_never_reach_generic_flows() {
        let data =
            json!({"orderId":"o1","receipt":{"email":"x"},"paymentInstructions":{"iban":"y"}});
        let consumer = public_event("consumer.order.confirmed", &data);
        assert!(consumer.get("receipt").is_none());
        assert!(consumer.get("paymentInstructions").is_some());
        let payment = public_event("payment.instructions.available", &data);
        assert!(payment.get("paymentInstructions").is_none());
        assert!(payment.get("receipt").is_some());
        assert_eq!(public_event("order.placed", &data), data);
    }

    #[test]
    fn the_rule_context_folds_the_event_into_the_order_states() {
        let order = json!({"cart":{"price":{"totalPrice":42}},"state":"open","payment":{"state":"paid"},"deliveries":[{"state":"shipped"},{"state":"open"}],"orderNumber":"1001","currencyId":"EUR","orderCustomer":{"id":"c"}});
        let event = json!({"orderId":"o1"});
        let context = rule_context("order.placed", &event, &order);
        assert_eq!(context["price"]["totalPrice"], 42);
        assert_eq!(context["eventKind"], "order.placed");
        assert_eq!(context["orderState"], "open");
        assert_eq!(context["paymentState"], "paid");
        assert_eq!(context["deliveryStates"], json!(["shipped", "open"]));
        assert_eq!(
            rule_context("x", &event, &json!({}))["deliveryStates"],
            json!([])
        );
        let definition = job_definition(
            &json!({"id":"f"}),
            "order.placed",
            &event,
            "o1",
            &order,
            &json!({"items":[]}),
            &context,
        );
        assert_eq!(definition["totalPrice"], 42);
        assert_eq!(definition["orderSnapshot"]["orderNumber"], "1001");
        assert_eq!(definition["cartData"], json!({"items":[]}));
    }
}
