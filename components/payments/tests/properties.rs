//! Property tests for the payment ledger: listed transitions only, no backward monetary step, captures and refunds within their bounds.
use proptest::prelude::*;
use veyndo_payments::domain::amounts::{format_amount, parse_amount};
use veyndo_payments::domain::refunds::checked_total;
use veyndo_payments::domain::state::transition;
use veyndo_pricing::verified_kernel::{capture_admissible, refund_admissible};

const STATES: [&str; 11] = [
    "pending",
    "ready",
    "approved",
    "authorized",
    "captured",
    "partially_refunded",
    "refunded",
    "cancelled",
    "expired",
    "captured_late",
    "partially_captured",
];

/// States in which money has moved; the ledger never leaves them for a pre-capture state.
const MONETARY: [&str; 4] = [
    "captured",
    "partially_refunded",
    "refunded",
    "captured_late",
];

/// The listed edges, written out independently of the extracted rule (Lean `payment_transition_exact`).
fn listed(current: usize, next: usize) -> bool {
    current == next
        || current <= 3 && next > current && next <= 4
        || current <= 3 && (next == 7 || next == 8 || next == 10)
        || [4, 5, 9].contains(&current) && (next == 5 || next == 6)
        || (current == 7 || current == 8) && next == 9
        || current == 10 && [4, 5, 6].contains(&next)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// A transition is admitted exactly when it is one of the listed edges; unknown states never move.
    #[test]
    fn only_listed_transitions_are_admitted(current in 0_usize..11, next in 0_usize..11, unknown in "[a-z_]{1,12}") {
        prop_assert_eq!(transition(STATES[current], STATES[next]).is_ok(), listed(current, next));
        if !STATES.contains(&unknown.as_str()) {
            prop_assert!(transition(&unknown, STATES[next]).is_err());
            prop_assert!(transition(STATES[current], &unknown).is_err());
        }
    }

    /// Any walk along admitted transitions never returns from a monetary state to a
    /// pre-capture one, and a refunded payment stays refunded (Lean `payment_refunded_terminal`).
    #[test]
    fn the_ledger_never_moves_money_backwards(choices in prop::collection::vec(0_usize..11, 0..40)) {
        let mut state = "pending";
        let mut moved = false;
        for next in choices {
            if transition(state, STATES[next]).is_ok() {
                if state == "refunded" {
                    prop_assert_eq!(STATES[next], "refunded");
                }
                state = STATES[next];
            }
            moved |= MONETARY.contains(&state);
            if moved {
                prop_assert!(MONETARY.contains(&state), "money moved back to {state}");
            }
        }
    }

    /// Lean `capture_exact` and `capture_bounded`: captures plus in-flight reservations never
    /// exceed the authorization, in any sequence of capture requests.
    #[test]
    fn captures_stay_within_the_authorization(authorized in 0_u64..1_000_000, requests in prop::collection::vec((0_u64..400_000, 0_u64..100_000), 0..12)) {
        let mut captured = 0_u64;
        for (requested, reserved) in requests {
            let admitted = capture_admissible(authorized, captured, reserved, requested);
            prop_assert_eq!(
                admitted,
                captured <= authorized && requested > 0 && reserved + requested <= authorized - captured
            );
            if admitted {
                captured += requested;
            }
            prop_assert!(captured <= authorized);
        }
    }

    /// Lean `refund_bounded` and `refund_sequence_bounded`: refunds never exceed the captured amount.
    #[test]
    fn refunds_stay_within_the_capture(captured in 0_u64..1_000_000, requests in prop::collection::vec(0_u64..400_000, 0..12)) {
        let mut refunded = 0_u64;
        for requested in requests {
            let admitted = refund_admissible(captured, refunded, requested);
            prop_assert_eq!(admitted, requested > 0 && refunded + requested <= captured);
            if admitted {
                refunded += requested;
            }
            prop_assert!(refunded <= captured);
        }
    }

    /// A confirmed provider refund delta is added exactly when it stays within the capture;
    /// a zero delta replays only a consistent total; negative amounts are always refused.
    #[test]
    fn provider_refund_totals_are_checked(captured in -10_i64..1_000_000, refunded in -10_i64..1_000_000, delta in -10_i64..1_000_000) {
        let total = checked_total(captured, refunded, delta);
        let valid = captured >= 0 && refunded >= 0 && delta >= 0 && refunded <= captured && delta <= captured - refunded;
        prop_assert_eq!(total.is_ok(), valid);
        if let Ok(total) = total {
            prop_assert_eq!(total, refunded + delta);
            prop_assert!(total <= captured);
        }
    }

    /// Provider wire amounts round-trip exactly at each currency's precision and are never negative.
    #[test]
    fn wire_amounts_roundtrip(minor in 0_i64..i64::MAX, code in prop::sample::select(vec!["EUR", "USD", "JPY", "KWD", "CHF"])) {
        let text = format_amount(minor, code).unwrap();
        prop_assert_eq!(parse_amount(&text, code).unwrap(), minor);
        let negative = format!("-{text}");
        prop_assert!(parse_amount(&negative, code).is_err());
    }
}
