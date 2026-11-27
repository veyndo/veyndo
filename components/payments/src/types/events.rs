//! The outbox events the payments component publishes: typed payloads behind `contracts/events/`.
use serde_json::Value;
use veyndo_core::event;

event! {
    /// The payment ledger recorded a provider state for an attempt; `payment.captured` marks the first capture.
    /// `order` is the order snapshot without the cart token.
    #[event(kinds = ["payment.captured", "payment.updated"], version = 1)]
    pub struct AttemptRecorded<'a> {
        order_id: &'a str => "orderId",
        attempt_id: &'a str => "attemptId",
        state: &'a str => "state",
        provider: &'a str => "provider",
        environment: &'a str => "environment",
        real_money_charged: bool => "realMoneyCharged",
        order: Value => "order",
    }
}

event! {
    /// Accepted recipient instructions (for example bank transfer details) became available with an invoice snapshot.
    /// Apps receive `order` and `paymentInstructions` only with customer and instruction data rights.
    #[event(kinds = ["payment.instructions.available"], version = 1)]
    pub struct InstructionsAvailable<'a> {
        order_id: &'a str => "orderId",
        attempt_id: &'a str => "attemptId",
        order: Value => "order",
        payment_instructions: Value => "paymentInstructions",
    }
}

event! {
    /// A customer revoked an active saved payment method.
    #[event(kinds = ["payment.instrument.revoked"], version = 1)]
    pub struct InstrumentRevoked<'a> {
        instrument_id: &'a str => "instrumentId",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        AttemptRecorded::SPEC,
        InstructionsAvailable::SPEC,
        InstrumentRevoked::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let order = json!({"id":"o"});
        let (environment, state) = ("live", "captured");
        let checks = [
            (
                AttemptRecorded {
                    order_id: "o",
                    attempt_id: "a",
                    state,
                    provider: "paypal",
                    environment,
                    real_money_charged: true,
                    order: order.clone(),
                }
                .payload(),
                json!({"orderId":"o","attemptId":"a","state":state,"provider":"paypal","environment":environment,"realMoneyCharged":environment=="live" && state=="captured","order":order}),
            ),
            (
                InstructionsAvailable {
                    order_id: "o",
                    attempt_id: "a",
                    order: order.clone(),
                    payment_instructions: json!({"iban":"x"}),
                }
                .payload(),
                json!({"orderId":"o","attemptId":"a","order":order,"paymentInstructions":{"iban":"x"}}),
            ),
            (
                InstrumentRevoked { instrument_id: "i" }.payload(),
                json!({"instrumentId":"i"}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
