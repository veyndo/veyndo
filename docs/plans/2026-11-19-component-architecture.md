# Component architecture: plan and checklist

Requested 19 November 2026. This is the working checklist for turning the single
root crate into complete components, typing the data, checking the SQL, removing
hidden dependencies, rebuilding both frontends as modular packages, extending the
test suite and proving the core rules in Lean. A checked task means the change is
merged and the named check is green; a plan or a partial move is not checked.

## Starting point (measured on 19 November 2026)

| Fact | Value |
| --- | ---: |
| Rust files in the root crate `src/` | 621 files, about 70,000 lines |
| Files that import everything with `use super::*` / `use crate::*` | 528 |
| Untyped `v["key"]` accesses | about 4,600 |
| SQL statements as run-time strings | 1,719 static, 42 built at run time |
| Tables | 151 |
| Component pairs that depend on each other | 77 |
| Direct accesses to another component's table | 256 |
| Proved rules / proved properties | 64 / see [proof coverage](../proof-coverage.md) |
| Admin frontend | 427 files, 17,700 lines of handwritten CSS, 6 shared UI parts |

[Component dependencies](../component-dependencies.md) holds the current numbers.

## Target

A component is one directory that contains everything belonging to it. The server
binary only composes components.

```
components/<name>/
  component.json     tables, permissions, events, MCP tools, required ports
  Cargo.toml
  src/
    lib.rs           the public interface, nothing else
    types/           ids, DTOs, enums (serde + schema)
    domain/          pure rules, no I/O; the part that is proved
    store/           the only place with SQL
    service/         use cases: transaction, rules, events
    http/  mcp/      thin adapters
    ports.rs         what the component needs from others
  migrations/
  tests/
  ui/admin/  ui/storefront/
apps/server/         binary: routes, wiring, workers
apps/connectors/
```

Rules that hold from the first component on:

- The monorepo is the source of truth during the move. Each component repository
  is a generated mirror that builds and tests alone with test adapters for its ports.
- A component depends on `core` and on the public interface of components below
  it. Cargo refuses a cycle; `components.json` records every remaining exception.
- Every move is a move only: no behaviour change in the same pull request.
- Files have no length limit. Clippy limits function length (`clippy.toml`).

## Component map

`components.json` is the checked version of this table.

| Component | Contains |
| --- | --- |
| core | errors, request context, database scope, outbox, admission, verified kernel |
| identity | merchant accounts, memberships, sessions, roles, route policy |
| shop | shop configuration, domains, legal texts, operator control plane |
| catalog | products, categories, media, reviews, translations, catalogue reads |
| inventory | stock, reservations, supply |
| pricing | money, price calculation, discounts, currencies, taxes, customer groups |
| shipping | delivery methods, destinations, delivery splits, carrier booking |
| checkout | cart, quote, checkout transaction, order placement |
| customers | customer accounts, sessions, addresses, customer directory |
| orders | order state, fulfilment, receipts, documents |
| payments | payment ledger, captures, refunds, provider workers |
| marketing | rules, coupons, promotions, flows, sales channels |
| b2b, pos | company procurement; point-of-sale adapters |
| apps | app packages, managed data, surfaces, Wasm extensions, app builder |
| automation | rule evaluation, flow graphs |
| intelligence | shop memory, knowledge, agents, proposals, inference |
| studio | overview, reports, staging shops, history |
| protocols | MCP, UCP, capability dispatch (adapters, stay with the server) |
| connectors | provider services and delivery queue (separate binary) |

## Phase 0: rules and measurement

- [x] Replace the 320-line file limit (Rust, proof modules, frontend) by a function
      limit and the component check.
- [x] `components.json`: every Rust source and every table has one owner
      (`scripts/components.py`, part of the structure check, with negative controls).
- [x] Dependency and table report: `docs/component-dependencies.md`.
- [x] `scripts/sql_prepare.py`: every static SQL statement is prepared against the
      migrated schema in the integration suite. First run: 1,604 pass, no mismatch.
- [x] Budgets in `components.json` that only fall: untyped field accesses (3,594)
      and files with SQL outside a `store` folder (328).
- [ ] Lower `too-many-lines-threshold` from 320 to 150 (35 functions to split).
- [x] Give the three statements whose parameter type comes only from the bound
      value an explicit cast, so they are covered too (1,607 statements covered).
- [x] Suite runner: `--keep-going` runs the remaining suites after a failure and
      lists every failed suite; the nightly workflow uses it.
- [x] Open an issue automatically when the nightly run is red (`nightly_report` job;
      first exercised by the next failing scheduled run).

## Phase 1: core and identity

- [x] `components/core`: error contract and its conversions as a crate.
- [x] Move the request context into `core`; its POS admission, host shop and
      access snapshot are core-owned data types now.
- [x] Move shop resolution, header access, request tracing, bounded HTTP bodies
      and the worker wake-up signal into `core`.
- [x] Move the outbox consumer (subscribers outside core arrive through the
      `Projector` port), channel metrics, the read cache and the database layer
      (deployment values, pools, row security, cluster leases) into `core`.
- [ ] Move request admission and the settings and language read caches out of
      `src/performance`.
- [ ] Replace the `App` state object by per-component state plus ports.
- [ ] Remove the nine dependencies `core` has on other components.
- [ ] `components/identity` with `types/`, `store/`, `service/`, `http/`.
- [x] Ban new `use super::*` / `use crate::*`: `glob_imports` per component in
      `components.json` only falls (528 files at the start); component crates have none.

Done when the server builds on both crates without glob imports from them.

## Phase 2: pilot `pricing`

- [ ] Merge `src/currencies`, taxes, customer groups and `crates/commerce` into
      `components/pricing` in the target layout.
- [ ] Money is one type in minor units everywhere; the column "uses floating
      point" on the proof coverage page is zero for this component.
- [ ] Typed DTOs at the interface; `sqlx::query_as!` with offline cache in `store/`.
- [ ] Write down what the pilot cost and what the template needs changed.

## Phase 3: the remaining components

Order follows the dependency report, lowest first. Each line is one move.

- [ ] catalog
- [ ] inventory
- [ ] shipping
- [ ] customers
- [ ] payments
- [ ] checkout
- [ ] orders
- [ ] marketing
- [ ] b2b
- [ ] pos
- [ ] apps (with `developer` and the vendored app storage)
- [ ] automation
- [ ] intelligence
- [ ] studio
- [ ] shop

Per component: sources moved, tables and migrations assigned, SQL only in
`store/`, typed interface, `foreign_tables` empty or each entry justified, tests
moved with the code, proof bindings updated.

## Phase 4: frontends

- [ ] F1 `tokens` and `ui`: design tokens, atoms and molecules on accessible
      headless primitives, with a component catalogue.
- [ ] F2 `api`: TypeScript client generated from the Rust types.
- [ ] F3 Admin shell: typed router with code splitting per component, query layer
      for server data, extension registry (navigation, route, page slot, dashboard
      card, form field) that first-party components use like apps do.
- [ ] F4 Move admin areas one by one, orders first; no new CSS in `admin/styles`,
      its line count only falls.
- [ ] F5 `storefront-kit`: product card, product page, cart, checkout steps and
      account, styled through tokens only. One typed contract per part yields the
      React component, the embeddable web element, the Visual Builder form and the
      MCP App. Rust keeps delivering the HTML; parts hydrate as islands.
- [ ] F6 Agent surfaces: MCP Apps generated from the same contracts; check the
      current OpenAI interface before committing to it. Purchase confirmation stays
      with the native checkout.

## Phase 5: bringing it together

- [ ] 5.1 The server is composition only: it builds its router and worker set from
      each component's `router()` and `workers()` and wires the ports. `src/`
      contains no business rule.
- [ ] 5.2 One contract source: OpenAPI, the TypeScript client, the MCP tool schemas
      and the API catalogue are generated from the component types; both frontends
      use only the generated client.
- [ ] 5.3 Migrations live with their component under one global order. A fresh
      installation and an upgrade from the last release produce the same schema
      (compared by a schema dump in the suite).
- [ ] 5.4 Mirrors: every component repository is generated from the monorepo,
      builds and tests alone, and `components.lock.json` records the released commit.
- [ ] 5.5 Acceptance of the whole system: all registered suites, the two-replica
      purchase and cancellation regression, a rolling upgrade (old and new binary
      on one database), and the hot-path benchmark compared with the starting point.
- [ ] 5.6 Production images still contain no Python interpreter; startup path and
      image contents checked, not only the build.

## Phase 6: removing the old

- [ ] Delete `src/prelude.rs`, every `#[path]` attribute and every compatibility
      re-export that only kept old call sites compiling.
- [ ] `uses` in `components.json` has no mutual pair; `foreign_tables` is empty or
      each entry carries a written reason.
- [ ] Remove the untyped-access budget once it reaches its floor; remove the
      run-time SQL strings that `query_as!` replaced.
- [ ] Delete the old frontend folders and the CSS they needed.
- [ ] Delete scripts and suites that tested the old layout only.
- [ ] Update the source map, module inventory, README, handbook and the website's
      architecture page; remove every path that no longer exists.
- [ ] Final dependency report: zero cycles, published with the release notes.

## Phase 7: test suite

Each component carries its own tests; the suite runner composes them.

- [ ] Unit tests for every function in `domain/`.
- [ ] Property tests for money, stock, discounts and state machines.
- [ ] Store tests per component against a real, isolated PostgreSQL database,
      including tenant isolation under forced row security.
- [ ] Port contract tests: one test set runs against the real implementation and
      against the test adapter, so the mirror repositories test the same behaviour.
- [ ] HTTP and MCP contract tests generated from the schema: every route and tool
      has a valid and an invalid case.
- [ ] Browser journeys for the admin and the storefront: purchase, refund, product
      edit while orders arrive, app installation, a non-English main language.
- [ ] Concurrency: two replicas, repeated delivery, lease loss, uncertain sends.
- [ ] Migration tests: fresh, upgrade, and the explicit legacy import.
- [ ] Mutation testing on `domain/`; a surviving mutant is a missing test.
- [ ] Coverage floors per component instead of one global floor.
- [x] The suite runner reports every failing suite of a run, not only the first.

## Phase 8: proofs

Lean proves the pure rules in `domain/`. It does not prove SQL, HTTP or providers;
those stay covered by the tests above, and the coverage page keeps saying so.

- [ ] Every function in `domain/` of core, pricing, inventory, checkout, payments
      and orders is either an extracted proved rule or listed as unproved with a
      reason. No third state.
- [ ] Money: addition and scaling never overflow silently; rounding is stated and
      proved; net plus tax equals gross; a discount never exceeds its base; a total
      is never negative.
- [ ] Price calculation: the order total equals the sum of its lines, shipping and
      tax for every input; the same input gives the same quote.
- [ ] Stock: a reservation never takes stock below zero; reserve followed by
      release restores the quantity; an order holds at most what it ordered.
- [ ] Checkout: an order is placed only for the reviewed cart revision and total;
      a repeated request places no second order.
- [ ] Payments: captured never exceeds authorised, refunded never exceeds
      captured, and every ledger transition is one of the listed ones.
- [ ] Orders: every reachable state of the order machine is a listed state, and
      completed or cancelled orders accept no further fulfilment.
- [ ] Access: a request scoped to one shop never satisfies a check for another;
      a missing permission never admits.
- [x] Signed offers: an expired offer or a changed cart, revision or total is never
      honoured (`offer_honoured`, four properties).
- [x] Sovereign AI mode admits only the self-operated provider on a self-hosted
      endpoint (`sovereign_request_admissible`, four properties).
- [ ] Extend the extractor to saturating addition and multiplication together with
      the first rule that uses them. The price calculation is still a floating-point
      port of the original calculators, so "total equals the sum of its parts"
      becomes provable only after phase 2 moves it to whole minor units.
- [ ] State-machine proofs as invariants over transitions, not only single steps.
- [ ] The comparison driver checks the compiled Rust rules against the Lean
      versions on generated inputs for every rule above.
- [ ] No `f64` on the money path.

## Order

Phases 0 to 3 run in sequence. Phase 4 runs in parallel from F1; F2 needs the
typed interfaces of the component it covers. Phase 7 and 8 tasks are done with the
component they belong to; their phases close when every component is through.
Phase 5 and 6 come last and end the work.
