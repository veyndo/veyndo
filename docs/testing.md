# Testing, coverage and maintenance contracts

## Fresh-install schema equality

The registered `schema_baseline` suite (`scripts/schema_baseline.py`) creates its own
database, runs the built `target/debug/veyndo` in `BOOTSTRAP_MODE=migrate` without demo
data and compares a normalized `pg_dump --schema-only` (every statement, sorted,
owners left out) plus the rows setup creates with `fixtures/schema/reference.sql`.
Timestamps, generated UUIDs and the per-database scope key are masked; everything else
must be identical. The reference was taken from a fresh installation of commit
`ce4285d7`, the last release with incremental migrations, so the component baselines
are proven to produce the same schema. A deliberate schema change regenerates it with
`--write`. There are no upgrade or legacy-import tests: a release replaces earlier
installations and setup refuses a database that an earlier release created.

## External order batching regression

The registered `marketplaces` suite calls the installed app's real native order
callback with 1, 20 and 24 lines. Its app-owned
[`order_localization.py`](../extensions/apps/marketplaces/tests/order_localization.py)
fixture checks frozen names, exact paid amounts, one native outbox event,
tenant separation, replay, failed-order rollback and concurrent final-stock
admission. Italian, short locale, parent/variant and blank-name cases preserve the
previous result. SQLx statement traces assert one translation query at each size
and zero on replay. A 25-line request remains rejected before persistence.

The regression was run against the preceding binary first: all behavioral checks
passed, and the new query-count assertion correctly rejected `[1, 20, 24]`.
The changed binary passes with `[1, 1, 1]`.
The strict `signed_runtime` suite repeats the real marketplace callback with
separate restricted system and tenant logins, signed request scope and forced
RLS. It asserts the same names, stock effects and constant query count.
[Measurement and delivery scope](evidence/external-order-batching-2026-10-23.md).

## Mail and connected app test ownership

The registered `email_tests`, `rust_connectors` and `connected_apps` suites keep
their existing names and execution order. Their shared fixtures and scenarios now
have separate owners:

| Owner | Responsibility |
| --- | --- |
| [email_tests.py](../scripts/email_tests.py) | Compose the same nine unittest cases from the mail fixture, protocol contracts and native journey. |
| [mail_contracts/wire.py](../scripts/testing/mail_contracts/wire.py) and [fixtures.py](../scripts/testing/mail_contracts/fixtures.py) | Disposable SMTP/HTTP servers, synthetic TLS certificates and isolated test settings. `rust_connectors.py` imports these fixtures directly. |
| [mail_contracts/contracts.py](../scripts/testing/mail_contracts/contracts.py) | Eight archived comparison cases for mail configuration, protocols, secrecy, retries and localized messages. |
| [mail_contracts/journey.py](../scripts/testing/mail_contracts/journey.py) | Actual Rust checkout, outbox, Flow Builder, app gateway and mail worker delivery to local SMTP, including HTTP/MCP permissions and sandbox denial. |
| [connected_apps.py](../scripts/connected_apps.py) | Run access, source import, event delivery and disconnect checks in their original order. |
| [connected_contracts/runtime.py](../scripts/testing/connected_contracts/runtime.py), [wire.py](../scripts/testing/connected_contracts/wire.py) and [client.py](../scripts/testing/connected_contracts/client.py) | Own process/server lifetime, local provider responses and scoped HTTP actions. Restore fixture environment variables after shutdown. |
| [connected_contracts/access.py](../scripts/testing/connected_contracts/access.py) | OAuth/PKCE, encrypted tenant binding, configuration revisions, channel tracking and access denial. |
| [connected_contracts/sources.py](../scripts/testing/connected_contracts/sources.py) | Private source import, retrieval/model evidence, token refresh and disconnect cleanup. |
| [connected_contracts/flows.py](../scripts/testing/connected_contracts/flows.py) | Native order/app events, durable Slack flows, idempotency, retries and uncertain delivery after restart. |
| [reference_connector.py](../scripts/testing/reference_connector.py) | Explicit test import boundary for the archived connector comparison implementation. |

The extraction preserves every mail test body, all 37 connected app assertions
and all nine reported scenarios. Before and after the change, the three registered
suites pass against disposable local PostgreSQL databases and provider fixtures.
The native connector suite retains its two process, encrypted configuration,
queue recovery and restricted database role checks. All provider calls stay local.

Run the affected suites through the existing registry:

```sh
python3 scripts/verify_integration.py --only email_tests rust_connectors connected_apps
```

`connected_apps` retains the archived connector as a differential fixture beside
the real Rust commerce process. The separate `rust_connectors` suite and native
mail journey exercise the shipping Rust service. Production ownership remains
documented in [Rust services](rust-services.md); no runtime moved into Python.

## Prototype feedback loop

Required checks on every change are short; the complete suite runs once a day and on
request. The handbook build and its link check are described in
[the documentation site](documentation-site.md).

### Daily full verification and short per-change checks

Since 29 October 2026, the prototype runs its complete test suite **once daily
at 02:17 UTC** on the default branch. **Run workflow** starts the same complete
suite manually, including on a selected branch when a change needs it. A normal
push or PR update no longer starts complete frontend coverage, Rust compilation,
Lean/mutation tests or PostgreSQL/provider integration.

| Trigger | What actually runs |
| --- | --- |
| Pull request update or main push | Short source structure, ownership, localization, API/source inventory, reviewed-source and prompt/documentation governance checks. No full tests or new coverage. |
| Daily schedule or manual workflow dispatch | Complete frontend tests/coverage, Rust/formal/differential checks, every native integration partition and same-source coverage admission. Existing coverage floors are unchanged. |
| Main publication or manually requested image | Actual image build and its small packaged-runtime smoke checks. These validate the artifact being published; they do not repeat the full suite. |

The required `verify` name stays in place for branch protection. On pushes/PRs,
it means **quick checks passed**, as stated in the run summary. It does not claim
that the current commit passed the full suite. Daily/manual runs must select the
full lane; failed, cancelled, missing or unexpectedly skipped jobs still reject
admission. Quick runs never download old coverage or label it as current evidence.

PR branches run through `pull_request`, without duplicate branch-push runs. A new
PR revision cancels its old quick run. Daily/manual full runs remain independent.
Documentation is built and checked when published from main or requested manually,
not again on every PR revision.

Local development follows the same cadence: run targeted tests for changed
behavior while fixing it, but do not repeat the whole suite for every commit or
wait on nightly tests to continue ordinary prototype work. Run another complete
suite only for an explicit acceptance request, a broad regression or a concrete
failure needing that scope. Record whether a result is targeted or complete.

`scripts/testing/ci_scope.py` still classifies actual Git diffs for image admission
and retains conservative full selection for scheduled/manual verification. Its
rename, missing-history and sensitive-change controls remain in the daily suite.

### Image publication scope

The preview and migration image workflows use the same complete Git comparison
with `ci_scope.py --images` before allocating a Docker runner. Only root
README/AGENTS/CHANGELOG, handbook/site assets and their rendering tools can skip
image work. Package-local Markdown, LICENSE/THIRD_PARTY, frontend, Cargo, fixtures,
runtime scripts, Compose, Dockerfiles, workflow changes and unknown paths still
build and smoke-test. The registered scope tests check the real Docker COPY inputs
against the skipped paths, including rename and multi-commit counterexamples.

Documentation publication and required verification remain independent. A docs-only
commit publishes the handbook without creating a new runtime image or moving its
tag. Missing history, empty changes and manual `workflow_dispatch` requests build.
If documentation lands while an earlier image is building, that tested image can
still advance the preview tag. Newer runtime changes, divergent history and an
unavailable comparison prevent promotion. The tag retains the actual built SHA.
Use **Run workflow** to rebuild intentionally; only the main branch can publish.
PRs touching image admission exercise the actual images without registry login or
publication. Existing packaged signup, private sandbox, session restart and non-root
Rust migration health checks still run whenever an image is built.

<a id="independent-frontend-feedback"></a>

### Complete daily run

After admission, complete frontend verification runs in parallel with the
Rust/formal/native jobs. The existing required `verify` job waits for all full-run
results, checks immutable coverage artifact IDs and rejects incomplete evidence.
These full jobs are skipped intentionally in the documented quick lane only.
No full daily scope or coverage floor was removed by changing the cadence.

Native integration uses three independent CI partitions with separate PostgreSQL,
Qdrant, files and ports on separate runners. `core` runs the ordinary registry;
`signed` runs `signed_runtime`, including its complete restricted-login HTTP
repetitions; `pooler` runs `signed_pooler` and `transaction_pooler`, including their
real one-backend PgBouncer checks. The single suite registry remains authoritative.
Partition validation rejects duplicates and missing special suites. A newly
registered suite automatically enters `core`; it cannot disappear between jobs.

The app-generation provider fixture additionally owns a fresh disposable database
inside its partition. Earlier suites intentionally leave durable tasks for recovery
tests; restarting all workers against that shared state could send unrelated
requests to a newly rebound loopback model. Generation keeps real workers and the
complete provider-wire assertions, starts with no unrelated model requests and
drops only its own uniquely named database after stopping its process.

The native collector waits for every partition. It checks source commit, workflow
run, producer attempt, Rust toolchain, absolute workspace paths, registry digest
and every raw profile checksum before copying any measurement. It rebuilds the
matching instrumented objects without rerunning tests. LLVM combines raw Rust
profiles and coverage.py combines Python measurements; no percentages are averaged
or manually added. The existing final policy and floors remain unchanged. Corrupt,
missing or substituted profiles fail the run. A failed matrix job prevents native
admission even if the other partitions succeeded.

Local verification remains sequential by default. `--partition core`, `signed` or
`pooler` selects a complete partition and rejects `--only`, `--existing-database`
and `--parallel-pooler`. The existing optional `--parallel-pooler` subprocess mode
is still available locally with its own disposable Qdrant endpoint. Its owned
process group, mandatory join and failure behavior are unchanged.

Run the registered `test_integration_partitions`, `test_partition_coverage` and
`test_parallel_check` tooling tests for complete partition admission, rejected
coverage substitutions and actual child-process cleanup. Each integration suite
continues to print its duration.

Each job uploads an immutable coverage artifact. The final job downloads the exact
IDs returned by those jobs, checks the checkout SHA, workflow run and SHA-256 of
each actual report, then runs the existing combined coverage policy and complete
source inventory checks. A successful job from an earlier attempt of the same run
can be reused when retrying failed jobs. Different commits/runs, future attempts,
missing reports and changed contents are rejected. Artifact names include attempts
so reruns cannot overwrite earlier evidence. Full HTML reports and integration logs
remain available beside the final combined summary; coverage floors are unchanged.

Run `python3 scripts/testing/test_ci_gate.py` for failed/skipped job and report
transfer controls. It is also registered in the native tooling suite. Workflow
syntax is checked with `actionlint .github/workflows/verify.yml`.
Frontend-only coverage reuses the same policy and source completeness check; it
does not fabricate fresh Rust/Python measurements or change their full-run floors.
Full runs retain both native separate-login and pooler/multi-replica regressions.
Each integration child logs its start and elapsed time, with unbuffered Python
output, so a slow suite is visible instead of hidden inside one long CI block.
The exact generated source inventory is checked before compilation or integration
tests, as well as in the existing structure gate. Missing inventory updates fail
immediately without waiting for the daily database suites.
The existing reviewed source/build-input registry is also checked before compiling.
The later complete formal proof, conformance and mutation checks remain required.

This optimizes development feedback. A green fast check is not a new whole-system
acceptance or evidence of live provider payments. The current prototype can use
short, coordinated deployment restarts; no zero-downtime fleet migration is required
for an ordinary UI or documentation release. Existing demo data is still retained.

## Source architecture

- Rust: 22 components declared in `components.json`, each with its own crate or source
  paths, tables and allowed dependencies ([architecture](architecture.md)). Every source
  file starts with a responsibility comment; file length is not limited. Clippy limits
  function length; `scripts/components.py` rejects undeclared or mutual component
  dependencies and foreign table access without a written reason.
- Frontend: `frontend/src/` holds the entry, shells, shared building blocks and `kit/`;
  screens live in `components/<name>/ui/{admin,storefront}/`. Runtime cycles, unresolved
  local imports, imports against the component dependencies, missing folder contracts
  and undocumented source files fail `npm --prefix frontend run architecture`.
- Bundled Email/GA4/Gmail/Slack implementations are Rust crates under
  `extensions/apps/<app>/`; service hosting is in `extensions/services/connectors/` and
  `extensions/services/connector-runtime/`. Python test tooling lives under `scripts/`;
  the former Python connector is an archived comparison oracle in `reference/`.
  [The generated inventory](module-inventory.md) covers runtime and tooling sources and
  is checked for drift.

`frontend/tests/architecture.mjs` also inserts deliberately broken synthetic imports,
a cycle, an oversized file and absent documentation into temporary directories;
it verifies that the architecture checker actually rejects each defect.

## Local Studio development account

Visual checks of Studio need a signed-in merchant. With the local server running
(`.claude/launch.json` entry `veyndo`, or `./scripts/dev.sh`), run:

```sh
python3 scripts/dev_account.py
```

The first run registers the shop `dev-shop` and its owner `owner@dev-shop.test`
through the ordinary `/api/auth/register` endpoint and stores the generated
password in the ignored `.run/dev-account.env` (mode 600); later runs only prove the
stored account still signs in. `DEV_ACCOUNT_PASSWORD` chooses the password and
`--shop`, `--name`, `--email` change the defaults. The script refuses any host other
than this machine and never prints the password. Sign in at
`http://127.0.0.1:8787/?shop=dev-shop#login`. After a frontend rebuild, run
`node scripts/precompress.mjs` too: the server prefers the precompressed sidecars.

## Browser journeys

`browser_journeys` (provider suite group) drives Studio and storefront in a real
Chromium: a guest purchase that then appears in Studio, a Studio refund of a
captured provider payment (local provider fixture), a product content edit saved
while another shopper's order changes stock (the stock decrement survives), an app
installation through its permission review, and a Spanish main language with Italian
as a fifth content language (the merchant's Italian name reaches the storefront;
untouched fields inherit Spanish and are not saved as translations). It starts its own
server on a free loopback port and registers one disposable shop per journey;
setup goes through the API, every step under test through the browser.

The driver is the pinned `playwright` package from `scripts/testing/requirements.txt`
plus `python3 -m playwright install chromium-headless-shell`; its bundled Node runs
`scripts/testing/browser_journeys.mjs`, so a working system Node is not needed.
`BROWSER_NODE` and `BROWSER_MODULE` select another Node and Playwright module. Run it
alone with `python3 scripts/verify_integration.py --container <postgres> --only
browser_journeys` after `cargo build --workspace --bins` and a frontend build. A
failing journey leaves a screenshot in `artifacts/browser-journeys/`.

## Component test suite (phase 7)

Phase 7 of [the component plan](plans/2026-11-19-component-architecture.md#phase-7-test-suite)
adds test sets beside the per-function domain tests. Each is listed with what it proves.

### Property tests

`proptest` (dev-dependency only; `scripts/testing/workspace_boundaries.py` refuses it as
a normal or build dependency) generates inputs for 37 properties in
`components/<name>/tests/properties.rs` and `components/marketing/src/store/promotion_property_tests.rs`.
Where a Lean property exists the test names it and states the same claim on the compiled
Rust; the others are open proof obligations of phase 8 tested here first.

| Crate | Properties |
| --- | --- |
| pricing (16) | currency scale bound; decimal text ↔ minor units bijection for every `i64` and scale; parsing refuses a wrong scale; half-away-from-zero rounding against an integer model; scaling error at most half a minor unit; checked addition exact or refused, never across currencies; float wire amounts round-trip in the exact range; negative/non-finite amounts have no minor units; `discount_cap` within base, requested, idempotent, monotone; discount allocation conserves cents, never takes a line below zero or above itself, monotone in the request; `amount_sum` exact or saturating and commutative; net plus tax equals gross; `math_round` idempotent, symmetric, within half a unit; quote totals add up for net and gross quotes; a line price is the rounded unit times the quantity, deterministic, tax within the gross |
| inventory (5) | `stock_admissible` exact; any sequence of reservations and releases keeps stock non-negative, conserves units, keeps every order within what it ordered, and releasing all restores the opening stock; inventory batches aligned and positive; reorder ceiling; an admitted supplier offer is never above the ceiling |
| orders (6) | every state reached from `placed` in a valid (app-extended) machine is listed and terminal states have no edge; validation admits only closed reachable machines; terminal orders refuse every action, including fulfilment and payment; the cancellation guard agrees with the blocker; completion needs payment and delivery; manual payments move only pending/authorized to paid |
| payments (6) | exactly the listed ledger transitions are admitted; no walk returns money to a pre-capture state; captures stay within the authorization and refunds within the capture over any request sequence; provider refund totals; wire amounts round-trip and are never negative |
| checkout (2) | the cart quote's total is the sum of its line totals (gross) or lines plus tax (net), net plus tax is the gross, the same cart gives the same quote; normalized quantities are on the step grid |
| marketing (2) | applied promotions stay within their base: no negative total, line or discount and no line rises; an exclusive promotion ends the chain and free shipping changes no amount |

Run them with `cargo test -p <crate> --test properties` (marketing: `cargo test -p
veyndo-marketing --lib promotion`). The order total's shipping part is added by the
checkout service (`service/delivery.rs`), not by a domain function, so the quote property
covers lines and tax; shipping is covered by the HTTP contract and checkout suites.

### Store isolation under forced row security

`store_isolation` (provider group) runs on the isolated suite database with a dedicated
login that is `NOSUPERUSER NOBYPASSRLS`, owns no table and has only row privileges, as the
production runtime role.

1. `tests/store_isolation.rs` calls component store functions through the scoped pool:
   it writes under shop A (outbox event, webhook receipt, product, stock reservation,
   price job, customer session; operator limits with system authority) and proves that
   under shop B the same functions read nothing (`exists`, `queued`, `find`, `valid`,
   cart locks of checkout, B2B, POS and marketing, orders, roles, settings, locale) and
   change nothing (updates affect no row, releases leave the reservation held, every
   insert for A is refused by row security). Fifteen components' stores take part.
2. A sweep over every tenant table each component declares in `components.json` (135
   tables in 16 components): under shop B no row of another shop is visible and an update
   or delete aimed at a holding shop touches no row; under that shop a row cannot be moved
   to another shop (row security `WITH CHECK`, SQLSTATE 42501). Every tenant table must
   force row security and belong to a component.
3. Negative control: a `BYPASSRLS` login sees shop A's products under the same probe.

On an isolated run only seeded tables hold rows, so update, delete and move probes are
meaningful for the tables that report "hold shop rows" (35 of 135 on the 25 November run);
the visibility probe applies to all. In the complete registry the suite runs after the
HTTP suites have filled the database.

### Port contract tests

`crates/port-contracts` (`veyndo-port-contracts`) holds one contract set per port and an
in-memory test adapter that meets it. The crate's own tests run every set against its
adapter; `src/port_contracts_tests.rs` and `src/payments/host_contract_tests.rs` run the
same sets against the server's real implementations (`cargo test --bin veyndo contract`).
A mirror repository depends on the crate and calls the same public functions with its
own implementation.

| Port | Contract | Test adapter | Real implementation |
| --- | --- | --- | --- |
| `veyndo_shop::ports::SettingsGuards::condition` | admits an always-valid condition; refuses unknown, non-string or missing types, non-objects and an unknown type nested in a container | `Conditions` | `commerce::Guards` |
| `veyndo_payments::ports::Host::service` | an unconfigured provider version is refused in every environment; a configured one answers the same service every time | `Services` | `payments::host::PaymentHost` |
| `veyndo_intelligence::ports::PublicEgress` | no client for plain HTTP, loopback, private, link-local, metadata or `localhost` destinations | `Offline` | `assets::Egress` |
| `veyndo_app_platform::ports::RecordRules` (pure methods) | records of an app without rules are stored as given, admitted, have no planning format, and normalizing is idempotent; an app with rules refuses its invalid fields and publishes a planning format | `NoRules` | `apps::data::BUNDLED` (site builder) |

Open: the transaction-bound port methods (`lock`, `capture`, `validate`, `captured`, …) and
the ports whose real implementation needs the server's `App` with a database (pricing,
inventory, checkout, orders, identity, customers, B2B, POS, studio, intelligence hosts).
The server is a binary crate, so its implementations are reachable only from its own unit
tests; running these sets needs a database-backed `App` fixture in those tests.

### HTTP and MCP contracts generated from the schema

`scripts/testing/schema_contracts.py` reads the route catalogue
(`frontend/src/kit/api/generated/routes.ts`, generated by `scripts/api_catalogue.py`) and the
operation contracts (`extensions/sdk/generated/operations.json`, generated by
`scripts/testing/operation_contracts.py`) and builds one valid and one invalid case per
route and per MCP tool. Its own check (tooling suite `testing/schema_contracts`) proves
every generated valid tool input satisfies its schema and every invalid one breaks it,
with negative controls. The HTTP suite `schema_contracts` runs the cases against the
running server on two disposable shops (session-ending routes last):

- HTTP valid: the route with path parameters set to a probe value, the shop's credential
  and, for body methods, `{}`. Invalid: a malformed JSON body for body methods, otherwise
  an invalid bearer credential. Six POST routes read no body (listed with reasons in
  `NO_BODY`; the list fails when a route leaves the catalogue) and get the credential case.
- MCP valid: the minimal arguments the input schema requires. Invalid: a missing required
  property, an unknown property of a closed schema, or a wrongly typed property.
- Assertions: no 5xx, except the documented 502/503/504 problem (retryable or
  `provider_unavailable`) of the dependencies the suite leaves offline; a malformed body is
  refused with 4xx; an invalid credential gets 4xx on every `/api/` route; every invalid
  MCP input is refused (JSON-RPC -32602 or a tool error with a 4xx problem).

25 November run: 350 routes (valid: 92 2xx, 251 4xx, 7 offline 503 from the unconfigured
identity broker, checkout identity and PayPal webhooks; invalid: 348 refused with 4xx), 212 tools (all
212 invalid inputs refused; of the valid inputs 50 succeed, 160 are refused for unknown
probe IDs or missing setup, 2 report the offline model service). No unexplained server error.

### Coverage floors per component

`coverage_report.py` maps every measured file to its component through `components.json`
(component UI under `components/<name>/ui/` counts for that component) and enforces a
line floor per component and scope (`components` in `coverage-policy.json`), beside the
global floors. A component measured without a floor, a floor with nothing measured, and
a value below its floor each fail; `--ratchet` raises floors to the measured values and
never lowers one. `--frontend-only` (every PR) enforces the frontend floors; the full
report (nightly) enforces both. Tests with negative controls are in
`scripts/testing/tooling_tests.py` (`ComponentCoverageFloors`); on the real policy,
dropping one POS module to zero hits fails the gate.

Floors of 25 November (Rust: workspace unit, property and contract tests instrumented
with `-C instrument-coverage` and LLVM 23, so the nightly run with the integration suites
measures more; frontend: vitest over this tree):

| Component | Rust lines | UI lines | | Component | Rust lines | UI lines |
| --- | --- | --- | --- | --- | --- | --- |
| apps | 41.70 | 69.53 | | inventory | 70.90 | – |
| automation | 75.13 | – | | marketing | 47.77 | 66.80 |
| b2b | 8.02 | 54.25 | | orders | 59.90 | 63.14 |
| catalog | 47.36 | 83.19 | | payments | 32.85 | 85.27 |
| checkout | 34.33 | 81.91 | | pos | 28.56 | 46.93 |
| core | 60.92 | – | | pricing | 81.57 | – |
| customers | 2.81 | 73.50 | | protocols | 42.34 | – |
| host | 10.56 | – | | shipping | 75.58 | – |
| identity | 21.52 | 73.59 | | shop | 52.92 | 61.79 |
| inference | 59.80 | – | | studio | 21.43 | 74.71 |
| intelligence | 18.68 | 79.27 | | | | |

Low Rust values (customers, B2B, host) are code reached only through the HTTP suites;
the nightly measurement covers it, and its first run should be ratcheted.

### Mutation testing of `domain/` (on demand)

`scripts/testing/mutation_domain.py` runs `cargo-mutants` (27.1.0, installed with
`cargo install --locked --root target/tools cargo-mutants`) on `src/domain/**` of pricing,
inventory, checkout, payments and orders against each crate's own tests, in parallel
copies of the tree (`--jobs`, default 8; about 20 minutes for all five on 32 cores).
Every surviving mutant must be listed with its reason in
`scripts/testing/mutation-survivors.json`; an unlisted survivor fails and so does a listed
one that is now caught. It is too slow for every run and, like the coverage audit, is
not in the suite registry: run it after changing `domain/` code.

```sh
python3 scripts/testing/mutation_domain.py [--crates pricing ...] [--jobs 12]
python3 scripts/testing/mutation_domain.py --check-only   # evaluate artifacts/mutants again
```

Run of 25 November 2026 (1,137 mutants):

| Crate | Mutants | First run: caught / survived | After the new tests: caught / survived | Unviable |
| --- | --- | --- | --- | --- |
| pricing | 341 | 227 / 101 | 325 / 3 | 13 |
| inventory | 51 | 45 / 6 | 51 / 0 | 0 |
| checkout | 192 | 123 / 67 | 186 / 4 | 2 |
| payments | 293 | 255 / 35 | 289 / 1 | 3 |
| orders | 260 | 224 / 34 | 256 / 2 | 2 |

The killing tests are `components/<crate>/tests/domain_mutants.rs` (52 tests). They pin
what the first run showed untested: inclusive limits of every validator, each clause of
`||`/`&&` chains on its own, `calculate`'s net input, duplicate tax rates and
list/regulation/reference prices, exact consumer and business quote amounts with app fees,
tiers, a 7 % product and a zero-decimal currency, IBAN bounds with valid checksums, and
order-machine limits. The ten remaining survivors, with reasons in the survivor list:

- pricing `math_round` (3): equivalent; the exact-hit correction is redundant because the
  half-unit check after it adds the same unit.
- checkout `cart_quote` (4): equivalent; `calculated: true` and `round_for_net: false` equal
  the defaults, and the cash interval `10^-decimals` never changes an amount already
  rounded to the decimals (line price and app fee).
- payments IBAN checksum (1): equivalent; adding 97 instead of reducing keeps the residue,
  and a reducing digit step always follows.
- orders (2): a re-encoded logo of exactly 2 MiB cannot be constructed; the NUL arm of the
  PDF text encoder is defensive (NUL is refused upstream) and observable only in a PDF.

## Repeatable checks

From the repository root:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --bins
npm --prefix frontend ci
npm --prefix frontend run format:check
npm --prefix frontend run build
npm --prefix frontend run architecture
npm --prefix frontend run localization
npm --prefix frontend run test:coverage
python3 scripts/formal.py
python3 scripts/formal/mutations.py
QDRANT_URL=http://127.0.0.1:16333 python3 scripts/verify_integration.py
```

Local integration creates and removes its own uniquely named synthetic database;
existing shops remain outside that run. The default database container is
`veyndo-postgres-1`; override it with `--container`. It uses `DATABASE_URL`
from the process or the local `.env`. CI uses the same owned disposable database
path. `--existing-database` remains an explicit local disposable-fixture override. Failures remain failures, and child processes stop
before database cleanup. SIGINT flushes optional Rust coverage profiles.

[scripts/testing/suites.json](../scripts/testing/suites.json) is the single
registry for HTTP suites, local provider and connector suites, browser contracts
and verification tools. It records the current suite names and execution order.
The local server uses an offline model URL; live model checks are separate, opt-in
checks. Credentials, payments, mail and Slack are exercised against loopback
protocol fixtures. Passing these does not demonstrate a real provider account.

The `currencies` suite exercises channel restrictions, Store API/MCP/UCP agreement,
fixed and converted prices, shipping/coupon conversion, stale checkout reviews,
immutable orders and grouped revenue. `payment_providers` tests create/capture/refund
with EUR/USD, zero-decimal JPY and three-decimal KWD against a local protocol fixture.
`production_foundations` denies cross-tenant currency-job SQL access using non-owner
roles, then restarts a worker after its first 100-product batch and verifies completion,
exact midpoint rounding and exclusion of products created after the job snapshot.
See [multi-currency commerce](currencies.md) for the configuration and proof boundaries.

The automation suite also executes `scripts/playground.py` as documented: a new
owner-owned shop, repeat setup after an edited rule, both checkout branches and
actual invoice records. `tooling_tests.py` checks refusal of remote credential
destinations and unsafe state file symlinks, alongside runtime ownership and
coverage reporting contracts. See
[the manual tour](playground.md) for the corresponding browser steps.

## Production boundary regressions

`production_foundations` runs two non-owner, non-bypass PostgreSQL replicas and
repeats native checkout, CRM and adversarial tenant checks. It rejects unsafe
TRUNCATE grants, omitted/forged tenant predicates, stale operator quotas and
corrupted stock snapshots; it tests commit-only cache eviction and daily AI usage
shared across live/staging. The 32-client scalability fixture explicitly configures
32 tenant permits. Production defaults and saturation policies remain configurable.
The [architecture guide](production-architecture.md) explains exactly what these
checks and the Lean subset establish.

## Component regressions

`frontend/tests/unit/` uses real components/hooks/transports, synthetic domain
fixtures, jsdom and Testing Library. Fetches fail by default unless explicitly
supplied by a test. Cases include:

- merchant authentication, chat failures, language changes, uploads and separate live/staging transports;
- navigation through permission-filtered lazy Studio workspaces and a signed-out login path;
- catalogue filtering, cursor pages, stale responses, customer-session expiry and forbidden operation replay;
- independent default addresses, linked customer/order/product back paths, configured translated groups, lazy history and confirmed revision-bound restoration with retained failed drafts;
- opt-in personalization, ranking, consent withdrawal and failed signals;
- product gallery, variants, normalized purchase quantities, tier prices, SEO, reviews and public source-backed product questions;
- delayed product answers after navigation and product-specific moderation with rejected writes;
- connected flow branches/deletion, frozen execution traces, incomplete JSON drafts and source condition round-trips in four languages;
- SMTP/Resend/SendGrid fields, saved secrets, read-only settings, translated templates and explicit dry-run test calls.

## Coverage collection

Frontend coverage includes **every** `frontend/src/**/*.{ts,tsx}`, every
`components/*/ui/**/*.{ts,tsx}` and the TypeScript/React modules owned by
`extensions/apps/**`, including modules never imported by a test.
HTML, JSON, LCOV and summaries go to
`frontend/coverage/`. App source globs resolve to absolute package paths and V8 external-file collection is enabled; the report rejects omitted app modules as well as omitted host modules. Styles and independent guest app bundles are outside that
statement metric and explicitly listed as separate scopes.

For Rust install `cargo-llvm-cov` 0.9.1 and `llvm-tools-preview`. Use the same
instrumented binaries for units and the actual HTTP
integration run:

```sh
export CARGO_LLVM_COV_TARGET_DIR="$PWD/target"
cargo llvm-cov clean --workspace
eval "$(cargo llvm-cov show-env --sh)"
cargo test --locked
cargo build --locked --bins
# Run the integration commands above, then:
mkdir -p artifacts/coverage
cargo llvm-cov report --json --output-path artifacts/coverage/rust.json
cargo llvm-cov report --html --output-dir artifacts/coverage/rust
```

Mutation variants write profiles outside the real-core report directory; deliberately broken code cannot inflate the production coverage metric.

The Rust report measures lines, functions and regions, not branch coverage. The
JSON preserves every module in this instrumented scope. Do not call covered
regions exhaustive branch coverage.

Python instrumentation is a development dependency, not a production service
dependency. It collects subprocesses and includes untouched namespace modules:

```sh
python3 -m pip install -r scripts/testing/requirements.txt
COVERAGE_PROCESS_START="$PWD/scripts/testing/.coveragerc" PYTHONPATH="$PWD/scripts/testing" python3 scripts/verify_integration.py
python3 -m coverage combine --rcfile=scripts/testing/.coveragerc
python3 -m coverage json --rcfile=scripts/testing/.coveragerc
python3 -m coverage html --rcfile=scripts/testing/.coveragerc
python3 scripts/testing/coverage_report.py
```

The report covers Python app/service sources and verification tools, so its
percentage must not be presented as coverage of the Python services alone.
Before a fresh standalone Python measurement, remove only the prior synthetic
`artifacts/.coverage*` data, to avoid mixing historical executions.

CI uploads complete reports and lists zero-hit modules and unmeasured scopes.
`coverage-policy.json` locks the measured regression floors; Studio transport, server-health and workspace-boundary
modules require 100% lines, branches, functions and statements.
Adding a source file counts in the denominator. Raising those floors is an
explicit reviewed change; do not silently lower them to obtain a green check.

## Explicit 100% audit and open work

```sh
npm --prefix frontend run test:coverage:full
python3 scripts/testing/coverage_report.py --require-full
```

These commands **currently fail**, as they should. The second also rejects a full
system claim while browser SDK/example-app JavaScript and Wasm instruction coverage
remain unmeasured. CSS requires visual behavior checks; live OAuth credentials,
external inference and production deployment need their own verification.

The per-component floors are in `scripts/testing/coverage-policy.json`. The largest remaining frontend gaps include order-detail edits/documents,
customer/address edge cases, full operator workflows, app iframe errors and
complex rule/flow interactions. Coverage HTML shows individual missed statements
and branches, not only totals. Future changes must add behavior regressions in
these owning modules and increase coverage; a passing threshold is not completion
of the 100% objective.

Lean source review locks and mutation checks continue in CI unchanged in scope;
see [formal verification](formal-verification.md) for precisely what is proved.

The account composition regression exercises both login and registration through
`CustomerAccount → CustomerSignIn → shopApi`. The simulated transport rejects
protected profile/address/order reads unless the form persisted the exact
tenant session key. This reproduces the former React-reserved `key` prop bug;
backend address/customer isolation is separately exercised over real HTTP.

Studio session regressions reproduce a product 401 after a previously loaded
overview, verify that all private controller state is cleared, and reconnect with
a renewed token. Shared merchant calls and focus revalidation exercise the same
path. Negative cases cover 403/5xx, customer/provider/operator errors, invalid
login attempts, old-token replies after reauthentication, coalesced reads, hidden
tabs, overlapping checks and timer/listener cleanup. Browser verification uses
the existing personal test account; no expiry bypass or new privileges are added.

## Adversarial tenant isolation

The registered `tenant_isolation` HTTP suite uses two synthetic workspaces, real
merchant/customer sessions, direct MCP/UCP calls, foreign IDs, forged scope
headers, same-ID app records and revoked one-workspace integration keys. It
re-reads victim state after denied mutations. The database helper tests all 22
new composite foreign keys with own/foreign controls, scans tenant foreign keys
for missing scoped counterparts, and tests forced app RLS under a restricted
role including context reset on the same connection. It runs automatically via
the existing CI integration registry, with zero provider calls.

[Exact scope and remaining core-RLS boundary](tenant-isolation.md). Grouped
checks are behavioral evidence, not 100% endpoint/authorization coverage.

## App platform suites

Registered PostgreSQL suites exercise `app_consent`, `app_callbacks`,
`app_preview`, `app_relations`, `app_schema`, `app_jobs`, `app_components`,
`app_assets`, `app_secrets`, `app_events`, `app_export` and `app_distribution`.
They observe installed versions and actual records/DDL, denied foreign mutations,
lease fences, provider signatures, replay and export bytes. Existing apps,
services, staging, connectors and automation suites verify compatibility.

Frontend tests exercise native code-behind, action allowlists, actor draft restore,
revision conflicts, iframe bundle rejection, controls and package consent. These
are regression evidence, not a claim of 100% app/host coverage. Full semantic
document tests additionally require the real Qdrant service; a missing retrieval
service must be reported as an unverified environment dependency, never replaced
by a mocked successful semantic result. See [the security matrix](app-security.md).

## App ownership checks

Architecture, localization, translation export and frontend coverage include trusted app UI under `extensions/apps/`. The Rust source inventory and verification review ledger include app and service Rust modules. `scripts/testing/plugin_ownership.py` checks canonical package layout and has rejecting controls for leaked core files and escaping symlinks. Provider and checkout integration suites continue to test existing HTTP/MCP/Flow behavior after relocations. This is source ownership and regression coverage, not a proof that every trusted compatibility adapter executes in a sandbox. See [exact boundaries](plugin-ownership.md).
