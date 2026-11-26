# Release notes

For the current implementation and acceptance state, use **[Status](status.md)**.
This page describes the current release candidate. Earlier releases are in the
[changelog](../CHANGELOG.md) and the dated records in [`evidence/`](evidence/).

## Component architecture — release candidate

Branch `refactor/component-architecture`. This is a
release candidate: it has not gone live, and the acceptance listed under
[verification](#verification-status) is not complete. The
[component architecture guide](architecture.md) describes the result; the
[component plan](plans/2026-11-19-component-architecture.md) is the checklist.

### What changed

| Before | Now |
| --- | --- |
| One root crate `src/` of about 70,000 lines; 528 files imported everything with `use super::*` | 22 components declared in `components.json`; 19 have crates of their own (17 under `components/`, automation and inference under `crates/`); the server composes them and holds the protocol adapters; no glob imports outside the vendored automation crate |
| 77 component pairs depended on each other; 256 direct accesses to another component's table | No mutual pair (Cargo and `scripts/components.py` refuse one); 47 port traits; 61 remaining foreign table accesses, each with a written reason |
| 1,719 SQL statements as runtime strings anywhere in the code | SQL only in `store/` modules; 1,267 queries checked at compile time from the committed `.sqlx` cache; 24 dynamic `QueryBuilder` queries remain |
| About 4,600 untyped `value["key"]` accesses | Typed views and request DTOs; four untyped accesses outside the vendored automation crate; a gate refuses new untyped readers |
| 110 incremental migration files | One baseline per component plus a cross-cutting foundation, applied in one global order; `schema_baseline` proves the schema equals the last incremental release |
| Studio and storefront screens in `frontend/src/{admin,storefront}` | Screens in `components/<name>/ui/{admin,storefront}` of 13 components; typed request paths; UI imports follow the component dependencies; design tokens only |
| Tests per area and one global coverage floor | Property tests, store isolation under forced row security, port contracts, generated HTTP and MCP contract cases, mutation testing of `domain/`, coverage floors per component, browser journeys |
| 64 proved rules | 74 proved rules with 217 properties; every commerce `domain/` function is bound to a proved rule or listed as unproved with a reason; a Rust/Lean differential suite |
| | Standalone mirrors of the 19 components with a crate, checked to build and test alone |

### What was removed

- The incremental migrations `migrations/001`–`113`, their checksum compatibility checks,
  the hosted-app backfill and the upgrade replay suites.
- The connector service's legacy import (`--import-legacy`) and
  `scripts/migrate_connector_state.py`, which carried state over from the former Python
  connector. The archived `reference/connectors-python/` stays only as the comparison
  oracle of the differential tests.
- Upgrade and cutover procedures in the handbook; they describe the fresh setup instead.
- `src/prelude.rs`, every `#[path]` attribute in `src/`, unused compatibility re-exports and
  the old frontend folders with the CSS only they needed.

### What operators must know

- **Fresh installation only.** This release replaces earlier installations. There is no
  upgrade path, rolling upgrade or legacy import, and data of a deployed earlier release is
  not carried over. Setup refuses a database that an earlier release created; set up a new
  database with `BOOTSTRAP_MODE=migrate`.
- The runtime contract is unchanged: a separate migration owner, a non-owner
  `NOBYPASSRLS` runtime login, `DATABASE_RUNTIME_URL` and `DB_RLS_REQUIRED=true`
  ([deploy safely](production-architecture.md#deploy-safely)).
- The connector service waits for its baseline `connectors/0001-baseline`, which the
  commerce setup applies. Its legacy import flag no longer exists.
- Demo data comes from `fixtures/demo/`; it is not part of a baseline.
- The restructuring moves code without intended changes to HTTP, MCP or UCP behaviour; the
  generated contract cases run against every catalogued route and tool. One deliberate
  hardening: an order machine snapshot that bypassed validation can no longer move an order
  into an unlisted state, and an edge out of a terminal state now answers 409
  "Invalid order transition" instead of 409 "terminalOrder".

### Verification status

Recorded for this release candidate:

- The required structure check: ownership of every source file and table, the dependency
  direction, the schema order and the falling ratchets.
- The suites of the areas each round of the restructuring touched, run on a fresh build in
  that round. The new suites ran when they were added: `schema_baseline`, `store_isolation`
  and `schema_contracts` ([testing](testing.md#component-test-suite-phase-7)),
  `formal_differential` with 72 rules and 228,195 cases
  ([proofs](formal-verification.md)) and `testing/component_mirrors_check`
  ([mirrors](component-mirrors.md)); the property tests and the mutation run of
  25 November.
- A complete run on 23 November passed 143 of 145 suites. The PHP comparisons against
  Shopware were removed on 26 November: Shopware compatibility is no longer a goal.
- The money changes (whole minor units on the calculation path, the exact decimal
  calculator) with the commerce, checkout, payment, currency, B2B, POS and
  `formal_differential` suites on a fresh build.

Open before the release is accepted:

- All registered suites in one run on a fresh build of the merged branch, the two-replica
  purchase and cancellation regression and the hot-path benchmark against the starting
  point (plan item 5.5).
- The production images checked by their contents and startup path, not only the build
  (5.6).
- The first CI run of `browser_journeys` and the first nightly run with the per-component
  coverage floors.
- The final dependency report published with these notes.

### Money

Prices, totals and taxes run in whole minor units (`Minor`, `Money`) with exact decimals
and rationals (`Decimal`, `Ratio`) on the calculation path. The calculator no longer
reproduces Shopware's binary64 steps: it computes exactly and rounds each amount once,
half away from zero. On the former comparison population 481 of 40,344 outputs changed
by one minor unit at ties. Money scaling, rounding, line totals and quote totals are
proved and bound ([formal verification](formal-verification.md),
[currencies](currencies.md)). The catalogue product snapshot still stores prices as
floating-point database columns; they are read into exact decimals at the boundary.
