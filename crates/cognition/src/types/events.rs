//! The outbox events the inference crate publishes: typed payloads behind `contracts/events/`.
use veyndo_core::event;

event! {
    /// An agent's model usage was reserved against the shop budget (`costMicros` in micro-units of `currency`).
    #[event(kinds = ["intelligence.agent.allocated"], version = 1)]
    pub struct AgentAllocated<'a> {
        agent_id: &'a str => "agentId",
        tokens: u64 => "tokens",
        cost_micros: u64 => "costMicros",
        currency: &'a str => "currency",
    }
}

/// The registry entries of this crate.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[AgentAllocated::SPEC]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let (tokens, cost) = (1200u64, 35u64);
        assert_eq!(
            AgentAllocated {
                agent_id: "a",
                tokens,
                cost_micros: cost,
                currency: "USD"
            }
            .payload()
            .unwrap(),
            json!({"agentId":"a","tokens":tokens,"costMicros":cost,"currency":"USD"})
        );
    }
}
