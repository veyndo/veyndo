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

## Records of other components

Marketing owns rules, promotions, flows, flow jobs and sales channels. It reads and
changes other components' rows only through their owners:

- Components it depends on are called directly on marketing's connection: catalog
  (products, categories, visibility, reviews, download grants, unit catalogue
  identity), shop (settings, overrides, hosted frontends), pricing (customer group
  automation), identity (preview sessions and memberships), core (a stage's live
  shop and shop activity) and B2B (a unit's listed products).
- Components that depend on marketing or that it does not use are reached through
  `ports::Records`, which the server implements in `src/ports/marketing.rs`: carts
  (checkout), customers and their sessions (customers), orders, their activity and
  receipts (orders), active app packages (apps) and work tasks (intelligence).
