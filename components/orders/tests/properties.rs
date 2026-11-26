//! Property tests for the order state machine: reachable states are listed, terminal states take no transition, payment or fulfilment.
use proptest::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_orders::domain::order_machine::{
    MachineEdge, MachineState, OrderMachine, OrderMachineValidation, default_machine,
};
use veyndo_orders::domain::order_transitions::{cancellation_blocker, payment};
use veyndo_orders::domain::order_workflow::guard;

fn labels() -> HashMap<String, String> {
    ["en", "de", "fr", "es"]
        .into_iter()
        .map(|l| (l.to_string(), "Step".to_string()))
        .collect()
}

/// The shape of an app extension: added states (terminal or not) and edges between state indexes.
type Extension = (Vec<bool>, Vec<(usize, usize)>);

fn extension() -> impl Strategy<Value = Extension> {
    (
        prop::collection::vec(any::<bool>(), 0..4),
        prop::collection::vec((0_usize..8, 0_usize..8), 0..12),
    )
}

/// The default machine extended by app states and edges, arbitrary (`valid` false) or
/// valid by construction: each added state is entered from an earlier non-terminal
/// state and further edges skip terminal sources, loops and duplicate pairs.
fn machine((extra, edges): Extension, valid: bool) -> OrderMachine {
    let mut m = default_machine();
    let edge = |n: String, from: &str, to: &str| MachineEdge {
        id: n,
        from: from.into(),
        to: to.into(),
        label: labels(),
        policy: None,
    };
    for (n, terminal) in extra.iter().enumerate() {
        let id = format!("app_state_{n}");
        if valid {
            let sources: Vec<String> = m
                .states
                .iter()
                .filter(|s| !s.terminal)
                .map(|s| s.id.clone())
                .collect();
            let from = &sources[n % sources.len()];
            m.transitions
                .push(edge(format!("app_enter_{n}"), from, &id));
        }
        m.states.push(MachineState {
            id,
            label: labels(),
            terminal: *terminal,
        });
    }
    let ids: Vec<String> = m.states.iter().map(|s| s.id.clone()).collect();
    for (n, (from, to)) in edges.into_iter().enumerate() {
        let (from, to) = (&ids[from % ids.len()], &ids[to % ids.len()]);
        let skip = from == to
            || m.states.iter().any(|s| &s.id == from && s.terminal)
            || m.transitions.iter().any(|e| &e.from == from && &e.to == to);
        if !valid || !skip {
            m.transitions.push(edge(format!("app_edge_{n}"), from, to));
        }
    }
    m
}

fn terminal(m: &OrderMachine, state: &str) -> bool {
    m.states.iter().any(|s| s.id == state && s.terminal)
}

fn order(state: &str, payment: &str, provider: &str, deliveries: &[&str]) -> Value {
    json!({
        "state": state,
        "payment": {"provider": provider, "state": payment},
        "deliveries": deliveries.iter().map(|d| json!({"state": d})).collect::<Vec<_>>()
    })
}

fn payment_state() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "pending",
        "authorized",
        "paid",
        "captured",
        "partially_captured",
        "partially_refunded",
        "refunded",
        "cancelled",
    ])
}

fn delivery_state() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["open", "shipped", "delivered"])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every state reached by a walk from `placed` along the edges of a valid machine is a
    /// listed state, and a terminal state has no outgoing edge, so a walk stops there.
    #[test]
    fn every_reachable_state_is_listed(shape in extension(), choices in prop::collection::vec(any::<prop::sample::Index>(), 0..30)) {
        let m = machine(shape, true);
        prop_assume!(m.validate().is_ok());
        let mut state = "placed".to_string();
        for choice in choices {
            prop_assert!(m.states.iter().any(|s| s.id == state), "unlisted state {state}");
            let next: Vec<&MachineEdge> = m.transitions.iter().filter(|e| e.from == state).collect();
            if terminal(&m, &state) {
                prop_assert!(next.is_empty());
                break;
            }
            if next.is_empty() {
                break;
            }
            state = choice.get(&next).to.clone();
        }
        prop_assert!(m.states.iter().any(|s| s.id == state));
    }

    /// Validation never admits an edge out of a terminal state, an unknown endpoint or an unreachable state.
    #[test]
    fn validation_admits_only_closed_reachable_machines(shape in extension()) {
        let m = machine(shape, false);
        if m.validate().is_ok() {
            let ids: Vec<&str> = m.states.iter().map(|s| s.id.as_str()).collect();
            for e in &m.transitions {
                prop_assert!(ids.contains(&e.from.as_str()) && ids.contains(&e.to.as_str()));
                prop_assert!(!terminal(&m, &e.from));
                prop_assert_ne!(&e.from, &e.to);
            }
            for id in ["completed", "cancelled"] {
                prop_assert!(terminal(&m, id));
            }
        }
    }

    /// Completed, cancelled and every other terminal order refuses every action: no
    /// fulfilment, cancellation, completion or payment (Lean `order_edit_admissible`).
    #[test]
    fn terminal_orders_accept_no_fulfilment(
        shape in extension(),
        pay in payment_state(),
        manual in any::<bool>(),
        deliveries in prop::collection::vec(delivery_state(), 0..3),
        kind in prop::sample::select(vec!["order", "payment", "delivery"]),
        target in prop::sample::select(vec!["cancelled", "completed", "paid", "shipped", "delivered", "in_progress"]),
    ) {
        let m = machine(shape, true);
        prop_assume!(m.validate().is_ok());
        let provider = if manual { "manual" } else { "stripe" };
        for state in m.states.iter().filter(|s| s.terminal).map(|s| s.id.clone()).chain(["expired".into(), "payment_review".into()]) {
            let mut o = order(&state, pay, provider, &deliveries);
            prop_assert_eq!(guard(&o, &m, kind, target), Some("terminalOrder"));
            if terminal(&m, &state) {
                let before = o.clone();
                prop_assert!(payment(&mut o, &m, "paid").is_err());
                prop_assert_eq!(o, before);
            }
        }
    }

    /// An open order is cancelled natively only with open money and open deliveries, and the
    /// action guard agrees with the cancellation blocker.
    #[test]
    fn cancellation_guard_matches_the_blocker(
        pay in payment_state(),
        manual in any::<bool>(),
        deliveries in prop::collection::vec(delivery_state(), 0..3),
        state in prop::sample::select(vec!["placed", "in_progress"]),
    ) {
        let m = default_machine();
        let o = order(state, pay, if manual { "manual" } else { "stripe" }, &deliveries);
        prop_assert_eq!(guard(&o, &m, "order", "cancelled").is_none(), cancellation_blocker(&o).is_none());
    }

    /// Completion needs ready money and every delivery delivered.
    #[test]
    fn completion_needs_payment_and_delivery(
        pay in payment_state(),
        deliveries in prop::collection::vec(delivery_state(), 0..3),
    ) {
        let m = default_machine();
        let o = order("in_progress", pay, "manual", &deliveries);
        let ready = ["paid", "captured"].contains(&pay) && deliveries.iter().all(|d| *d == "delivered");
        prop_assert_eq!(guard(&o, &m, "order", "completed").is_none(), ready);
    }

    /// A manual payment moves only from pending or authorized to paid, on a non-terminal order.
    #[test]
    fn manual_payment_marks_only_open_payments(
        pay in payment_state(),
        state in prop::sample::select(vec!["placed", "in_progress", "completed", "cancelled"]),
        target in prop::sample::select(vec!["paid", "refunded", "pending"]),
    ) {
        let m = default_machine();
        let mut o = order(state, pay, "manual", &[]);
        let admitted = payment(&mut o, &m, target).is_ok();
        prop_assert_eq!(admitted, !terminal(&m, state) && ["pending", "authorized"].contains(&pay) && target == "paid");
        prop_assert_eq!(o["payment"]["state"].as_str(), Some(if admitted { target } else { pay }));
    }
}
