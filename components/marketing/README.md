# veyndo-marketing

Rules, coupons, promotions, flows and sales channels. The server routes in
[`src/marketing`](../../src/marketing/README.md) delegate to this crate; the
automation guide in [`docs/automation.md`](../../docs/automation.md) describes
the contracts.

## Open JSON

Stored and wire documents are read through typed views and written through typed
patches (`veyndo_core::document::{view, merge}`). These untyped accesses remain
on purpose:

| Place | Reason |
| --- | --- |
| `src/domain/rule_match.rs`, `Condition::checked_at` (`Reference`) | Rule facts are keyed by the referenced rule's id, which the merchant declares; the lookup `q["ruleFacts"]["rules"][rule_id]` addresses an arbitrary declared key. |
| `src/domain/rule_comparison.rs`, `bounded` | The same reference lookup by declared rule id while bounding nested references of a historical comparison. |
| `src/domain/rule_match.rs`, `Condition::checked_at` (`Source`) | `q["ruleFacts"]` is handed unchanged to the vendored source rules engine (`veyndo_automation::rules::evaluate`), whose input is the whole fact document. |
| `src/domain/rule_match.rs`, `Condition::checked_at` (`EventField`) | Event field conditions walk a merchant-declared dot path through the event document, starting at `q["event"]`. |
