# Native controlled experiments

`mod.rs` owns the permission checks and passes the shop's enabled currencies and
the consent-locked cart to the store. The intelligence component owns the rest:
`components/intelligence/src/domain/experiment_model.rs` validates immutable
preregistrations and conservative bounded-outcome intervals;
`components/intelligence/src/store/experiments.rs` with `experiments_report.sql`
holds the lifecycle transitions, the layout assignment, the worker steps and the
readout from the canonical live payment/refund ledger, which saves one mature
report and projects it into existing knowledge relations/outbox events. No
separate analytics or payment store exists.

See [the contract and assumptions](../../../docs/cognitive-commerce.md). Rust unit
tests, the registered `cognitive_experiments` HTTP/PostgreSQL suite and the native
experiment-result Lean predicate cover distinct boundaries. Synthetic receipt
fixtures do not establish a real causal commerce effect.
