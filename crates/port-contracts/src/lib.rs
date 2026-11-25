//! Contract test sets for component ports: each module states what every implementation of one port must do and ships an in-memory test adapter that satisfies it.
//!
//! The server runs the same sets against its real implementations (`src/port_contracts_tests.rs`);
//! a mirror repository runs them against its own implementation by calling the public functions.
//! A contract panics with the port and the violated rule.
pub mod payment_host;
pub mod public_egress;
pub mod record_rules;
pub mod settings_guards;

/// A transaction-bound port method of a test adapter: admits without touching a database.
pub(crate) fn admitted<'a>() -> veyndo_core::PortFuture<'a, ()> {
    Box::pin(async { Ok(()) })
}
