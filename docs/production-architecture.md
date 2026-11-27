# How Veyndo works: the architecture

VEYNDO has one deterministic Rust commerce core, built from 22 components that each own
their tables, rules and screens ([component architecture](architecture.md)). The
storefront, merchant Studio, API clients and MCP/UCP adapters call the same component
operations. AI helps find information,
explain behavior and propose changes; it does not decide whether a payment is
received, a user is authorized or stock can be sold.

This guide describes the executable architecture, its current protection boundaries,
and the next migrations. It complements the [feature tour](features.md),
[feature overview](features.md), [module inventory](module-inventory.md)
and [test coverage](testing.md). Diagram images open at full size.

[![System map: experiences, admission, deterministic commerce, PostgreSQL, apps and workers](assets/architecture/system.svg)](assets/architecture/system.svg)

## Follow a real request

1. The browser selects a shop and sales channel. A hosted subdomain resolves to a
   stored frontend binding. Domain, tenant existence, staging parent, live status and
   selected channel are loaded in one fresh SQL snapshot and retained for this
   request; the proxy cannot remap an admitted mount through a later lookup. A sales channel is an experience within a merchant
   workspace; an unrelated merchant is a different tenant, not another channel.
2. Authentication strips caller-supplied internal identity headers, resolves the
   current session, all grants, shop status and selected channel in one authoritative
   query, and checks explicit route/method permission. Only native public shell/assets
   skip identity SQL; hosted shop assets and previews retain admission. The trusted Principal lives in request extensions. Customer
   operations also verify the customer/cart/download capability.
3. Resource admission takes bounded process and tenant permits. Checkout uses a
   separate process pool, so catalog traffic cannot consume all checkout permits.
   Interactive AI admission also reserves a durable daily quota before the handler.
4. A task-local database scope follows the admitted request. Fresh and reused
   SQLx connections receive that scope before queries; explicit transactions use SET LOCAL. Session mode rebinds every borrow without an extra reset query; transaction-pool mode scopes direct queries as well. Unknown tasks receive an empty, denied context.
5. The component's HTTP handler validates its entity IDs, rights and expected revision
   and calls a service of that component. The service runs the pure `domain/` rules and
   the checked SQL of its `store/`, reaching other components through their stores or
   declared ports. Mutations use PostgreSQL transactions and write their outbox events in
   the same commit. MCP and UCP use these operations rather than a separate payment engine.
6. Workers consume the committed events. They update read models, project knowledge,
   evaluate flows, deliver app events or execute payment commands. Slow model/provider
   calls are outside checkout locks. The browser receives the native API result.

## What lives where

| Responsibility | Actual implementation | Authority |
|---|---|---|
| Merchant identity and permissions | `components/identity/` (sessions, roles, MFA, route policy), `src/auth/middleware.rs` | Current PostgreSQL session/membership; no client principal |
| Tenant-scoped database leases | `components/core/src/request_context.rs`, `crates/runtime/src/tenant_scope.rs`, `crates/runtime/src/scoped_pool.rs`, `components/core/src/store/{pool,row_security}.rs` | Server-derived scope; PostgreSQL row policies |
| Catalogue, cart and checkout | `components/catalog/`, `components/checkout/src/service/purchase/{lock,price,commit}.rs` | Locked catalog/settings/cart snapshot |
| Exact money boundary | `components/pricing/src/domain/money.rs` | Checked minor integers and explicit currency scale |
| Stock allocation | `components/inventory/src/store/reservations.rs`, `components/inventory/migrations/0001-baseline.sql` | Persisted quantities linked to tenant + order + product |
| Orders | `components/orders/src/domain/order_workflow.rs`, `components/orders/src/store/` | Proved order state machine, immutable amounts and snapshots |
| Payments | `components/payments/src/{domain,store,service}/`, `src/payments/` (host port and provider routes) | Verified receipts, immutable provider identity, fenced durable jobs |
| Rules, flows and app events | `components/marketing/`, `components/apps/`, `crates/automation/`, `src/outbox.rs` | Versioned contracts and current action permissions |
| Read caching, admission and delivery | `components/core/src/{admission,cache}.rs`, `src/performance/` | Immutable Arc handles; current SQL versions/access snapshots; outbox eviction; secret-free compression |
| Shop memory and retrieval | `components/intelligence/` (including `knowledge/`), `src/cognition/` | Provenance/publication filters and persisted event receipts |
| Operator controls | `components/shop/` (operator control), `src/platform/` | Separate personal operator grant, audit and revisions |
| Protocol adapters | `src/mcp/`, `src/ucp/`, `src/capabilities/` | Same component operations as HTTP; no own business rule |
| Schema setup / startup | `src/migrations.rs`, `src/migrations/schema.rs`, `components/*/migrations/`, `src/bootstrap.rs` | Ordered component baselines recorded with checksums, fresh installation only, separate migration credential |

The [complete module inventory](module-inventory.md) includes every source file.
The [source map](source-map.md) maps additional behavior to its real tests.

## Checkout, stock and monetary truth

[![Checkout transaction and provider protocol, including cancellation and late capture](assets/architecture/checkout.svg)](assets/architecture/checkout.svg)

The final checkout locks the cart, product rows and effective settings. It recomputes
price/tax/shipping/promotions and checks the cart revision and reviewed amount.
An idempotency key identifies the purchase, not simply the HTTP request.

The same transaction writes the order, deducts available stock, creates an allocation
for every order line and writes `order.placed`. This allocation now exists for manual,
simulated and provider-backed methods. Cancellation uses its persisted quantity,
not an editable line-item snapshot. Setting `released_at` and restoring stock happen
atomically in one statement; repeat cancellation cannot release stock twice.
Deduction and allocation are batched per cart. Product locks remain in sorted ID
order for checkout and release, including baskets supplied in opposite orders.

Provider-backed checkout also creates a payment attempt and durable command. Its
provider version, merchant account, environment, currency and amount are frozen.
The worker claims a lease, commits the claim, calls the provider and applies the
verified receipt in a fenced transaction. A timeout can mean that the provider
already charged money: it leaves reconciliation work, not an automatic paid flag
or automatic stock release.

The central payment state machine admits:

- Pending → ready → approved → authorized → captured, with valid skips for
  provider contracts that capture immediately.
- Uncaptured cancellation/expiry, with explicit void confirmation for an authorization.
- Captured → partial refund → full refund; terminal refund never returns to ready.
- Cancelled/expired → late capture → payment review, with an explicit refund path.

Amount, currency, account, signature and receipt allocation checks remain separate
from state admission. The state graph alone cannot prove that an external charge
occurred. Local provider fixtures cover authorization/void, capture, lost replies,
partial refunds, restart and duplicate notifications. Real provider credentials,
merchant onboarding and live acquiring require separate integration evidence.
Shopware Payments implementation and attribution remain in the private repository;
the public core exposes the [generic payment contract](payment-provider-api.md).

### Money: exact boundary, deliberate migration

New `Money` values contain an `i64` minor amount and a `Currency { code, scale }`.
For example, EUR 12.34 is `1234` with scale 2; JPY 1234 uses scale 0; KWD 1.234
uses scale 3. Parsing/formatting uses integer arithmetic, rejects ambiguous precision,
and supports checked addition only for matching code **and** scale. Scale is bounded
to six; signed credit amounts including `i64::MIN` round-trip without overflow.

The active checkout supports a channel-selected currency. Orders include an explicit
`money` snapshot; the payment ledger persists its currency scale (including zero
and three). [The multi-currency guide](currencies.md) explains automatic/fixed
pricing, saved rates, protocol contracts and tested provider boundaries.

The calculation path works in whole minor units (`Minor`/`Money` in `components/pricing`)
and exact decimals; the price calculator computes exactly and rounds each amount once,
half away from zero, to the currency exponent, so a decimal tie is decided by the
decimal, not by binary error. The checkout boundary rejects non-finite, negative or
out-of-range payment totals. Rounding is stated and proved in Lean (`line_total`,
`rounded_quotient`, the calculator's cash-interval rounding, quote totals); see
[formal verification](formal-verification.md). Shopware compatibility is not a goal.

## Independent SaaS shops and row security

[![Verified tenant identity, scoped database lease, FORCE RLS and explicit privileged exceptions](assets/architecture/security.svg)](assets/architecture/security.svg)

The component baselines install `ENABLE` and `FORCE ROW LEVEL SECURITY` on public
core tables with a tenant column. Managed app tables keep their existing forced
app policy. New tenant tables must add equivalent policies, scoped foreign keys and
adversarial tests. Strict startup checks coverage, the effective runtime role,
owner-role membership, `BYPASSRLS`/superuser status and core `TRUNCATE` privileges.

**Enable strict runtime enforcement with both `DATABASE_RUNTIME_URL` and
`DB_RLS_REQUIRED=true`.** Migration owner and runtime credentials must differ.
PostgreSQL superusers and `BYPASSRLS` roles bypass row security even when forced;
installing policy SQL while continuing to use that role does not secure a deployment.
See the [PostgreSQL row-security documentation](https://www.postgresql.org/docs/current/ddl-rowsecurity.html).

The runtime uses task-local tenant context and the shared `ScopedPool` executor.
Explicit transactions batch BEGIN and SET LOCAL; transaction-pool mode also wraps
direct queries in a scoped transaction. Session mode binds every checkout, with no
second reset query on return. Unknown tasks fail closed. Transaction pooling is
verified through actual PgBouncer with one backend, protocol prepared statements
and two Rust replicas. It needs a separate direct/session `DATABASE_LISTENER_URL`;
statement pooling is unsupported. [Security boundaries](security.md).

Identity resolution, authenticated platform operations, registration/provisioning,
staging/developer control paths and fleet workers deliberately use trusted system
scope. Their existing ownership checks still matter. Ordinary shop business requests
receive a tenant scope. Apps receive capability contracts, never the migration URL.
App definitions are logical rows in fixed shared tables; runtime logins need no
app DDL or table ownership. See [app storage](app-data-storage.md). This is defense in depth, not a proof against SQL
injection, a compromised process, all side channels or arbitrary external services.

### Deploy safely

1. Set up a new database with `DATABASE_URL` using `BOOTSTRAP_MODE=migrate`; it applies
   the component baselines. A release replaces earlier installations, so there is no
   upgrade of an existing database.
2. Provision a dedicated runtime login through the database/secret manager
   ([runtime-role.sql](../deploy/sql/runtime-role.sql) creates the passwordless group).
   Then run `veyndo --provision` as the migration owner: it grants Core DML to the group
   (never connector tables), connector tables to the connector login, configures and
   activates the database authority and verifies the result. Every later migration grants
   the tables a release adds. Shared app tables stay with the migration owner. Never grant
   the migration owner to runtime.
3. Set `DATABASE_RUNTIME_URL`, `DB_RLS_REQUIRED=true`, `BOOTSTRAP_MODE=serve` and
   `ALLOW_BOOTSTRAP_AUTH=false` for public HTTP and worker deployments. Membership
   in `veyndo_runtime` must be inherited by the runtime login. All fixed Core/app tables stay with the migration owner. Separate signed tenant/system
   logins use the [authenticated scope setup](authenticated-database-scope.md).
   Inventory inherited secrets before detaching an owner group: preserve the existing
   `PLATFORM_SECRET_KEY` and unrelated provider/app configuration. Do not rotate
   encryption keys as a side effect of changing database credentials. The HTTP/worker
   environment must not retain the owner addon URI. See the dated
   [hosted transition and live checks](managed-hosting.md#restricted-hosted-runtime).
4. Reserve `DB_POOL_MAX + 2` per process for the pool and listeners against the shared `DB_CLUSTER_CONNECTION_BUDGET`. A fenced heartbeat enforces this fleet budget; a lost lease stops its process.
5. Run the strict `production_foundations` suite on a disposable database and test
   registration, ordinary and sandbox operations, callbacks and cold restart before
   switching public traffic. Public hosting secrets/configuration are operational
   work; a local regression does not establish that the public deployment uses RLS.

## Caches: commit-driven eviction, version-driven correctness

Decoded settings are bounded by entries and estimated bytes. Request memoization
avoids duplicate context work within one read request. Keys contain tenant/channel;
a primary-database version probe still validates every new request.

Committed outbox insertion produces a PostgreSQL notification containing only its
tenant identifier. Each HTTP/worker replica has a listener and evicts that tenant's
settings entries. Configuration writes generate explicit `cache.invalidate` outbox
rows. Rollback emits no notification. Missed notifications cannot grant stale values:
a version probe rejects them; reconnect also clears cached entries. Language cache
correctness still uses the authoritative registry version. This is eager invalidation
without making transient notification delivery the authority.

The listener is a performance hint. Business outbox delivery remains durable and
transactional. Do not replace it with a volatile event bus or a cursor that assumes
sequence IDs commit in numerical order. No public personalized page, cart, token,
customer response or authenticated MCP result is shared through a public CDN cache.
See [read performance](read-performance.md) for the measured baseline and limits.

## Admission, quotas and observability

| Configuration | Default | Meaning |
|---|---:|---|
| `HTTP_CONCURRENCY` | 128 | Process permits for ordinary API requests |
| `CHECKOUT_CONCURRENCY` | 32 | Separate reserved process permits for checkout/payment/UCP |
| `TENANT_CONCURRENCY` | 16 | Concurrent API requests per tenant across replicas (plus local defense) |
| `TENANT_AI_CONCURRENCY` | 2 | Interactive AI requests per tenant across replicas (plus local defense) |
| `TENANT_AI_DAILY_QUOTA` | 1000 | Default durable admitted interactive AI attempts per UTC day |
| `DB_POOL_MAX` / `DB_POOL_WAIT_MS` | 20 / 5000 | SQL connection capacity and bounded queue wait |

Admission uses immediate permits and returns 429 plus `Retry-After` on saturation;
permit drop also handles cancellation. Tenant bookkeeping is bounded to 4096 active/
recent tenant budgets, and unvalidated callers share an anonymous bucket, so arbitrary tenant headers cannot allocate new entries.
Checkout's separate process capacity still shares the configured tenant cap and SQL
pool: this is not a guarantee of unlimited checkout throughput.

The daily interactive AI quota is an atomic PostgreSQL reservation shared by replicas.
A private stage charges its live workspace through a narrowly scoped quota query;
sales channels already share that same tenant.
Failures count once admitted; a timeout may already have consumed provider resources.
Platform operators configure a shop through `GET/PUT /api/platform/shops/{id}/quotas`:
`{ "revision": 1, "dailyAi": 200 }`. Revisions reject lost updates; changes are audited.
This quota counts `/api/experience`, `/api/concierge`, agent chat/plan and product
`/questions` and `/ask` HTTP routes, not tokens, actual
provider spend, every MCP-derived AI action, background translation/image jobs or
identity-broker usage on behalf of another shop. The actual model boundary also has shared concurrency leases for translation, image, developer and broker work; these limit concurrent calls, not tokens or spend. [Dedicated planning-agent keys](agent-authority-controls.md) add persistent per-key token and USD allocation envelopes at an explicit provider/model/tariff, plus a personal live-shop stop shared with its sandboxes. Cognition uses the pure kernel allowance guard; it does not import commerce handlers. This is not actual invoice reconciliation or a complete tenant AI cost cap.

`/api/platform/infrastructure` exposes bounded latency histogram buckets, calls,
failures, capacity and rejection counters, SQL pool/read-cache diagnostics and
persisted channel traffic. Bootstrap-only `/api/runtime` remains a local diagnostic;
public hosting should use the personal operator interface. Histogram and concurrency
values are process-local; persisted channel counts are fleet diagnostics. They are
not visitor counts, and the buckets are not measured p95/p99 values. Process restarts
reset these local counters. No raw URLs, customer data or tokens are metric labels.

## How shop intelligence is connected

[![Products and documents, committed observations, graph/vector retrieval, grounded model and reviewed action](assets/architecture/intelligence.svg)](assets/architecture/intelligence.svg)

Product edits and knowledge ingestion store source-linked graph data in PostgreSQL.
Configured embedding workers separately index retrieval vectors in Qdrant. Search
uses tenant/model filters and hydrates candidates through current database and
publication checks. An embedding result is a candidate, not authorization.
A rolling model change uses a higher revision per shop so old workers
cannot replace the new target or publish an earlier result.
[Embedding model rollout](knowledge-component.md#change-the-core-embedding-model).

Committed order/payment events pass an event-receipt deduplication fence. Confirmed
provider captures produce observed product pairs, evidence-linked relationships and
reviewable bundle hypotheses. Uncaptured external payments do not count as captured
purchases. Simulated observations remain labelled. Personalization records session
exposures and rewarded outcomes; there is no demonstrated causal sales lift here.

The assistant receives bounded, scoped context and calls the configured model.
Current AI integrations do not change model weights after transactions. "Learning"
here means persisted observations and reusable context, not online training or proof
of economic benefit. A proposed action still needs rights, an expected revision and
a deterministic native operation. Flows can invoke configured AI/app actions under
the same declared capabilities; provider output cannot become an unchecked SQL command.

## What the gates actually establish

- Rust tests cover exact scale/amount boundaries, signed overflow, mismatched currencies,
  scoped task restoration, semaphore cleanup and monotonic payment transitions.
- `production_foundations` runs two real Rust replicas under a non-owner, non-bypass
  database login, deliberately omits tenant predicates, attempts foreign mutations,
  repeats checkout/CRM/tenant regressions and checks commit/rollback cache events.
- `transaction_pooler` repeats the strict-runtime suite through a real one-backend PgBouncer; poison-event, retention, login-backoff and shared-limit controls also run in both modes.
- Payment suites use local provider fixtures for restart, capture/refund, duplicate
  notifications, version/account pinning, authorization/void and uncertain receipts.
- `store_isolation` calls the store functions of 15 components and sweeps all 135 tenant
  tables under a non-owner `NOBYPASSRLS` login: a second shop sees and changes nothing.
- Property tests (37), mutation testing of `domain/`, generated HTTP and MCP contract cases
  (`schema_contracts`), per-component coverage floors and browser journeys cover the
  components; `schema_baseline` proves a fresh installation equals the reference schema.
  [Component test suite](testing.md#component-test-suite-phase-7).
- Lean proves 74 rules with 217 properties, including currency precision, the per-order
  stock bound, the order state machine over request sequences, capture and refund bounds
  and ledger transition admission ([proof coverage](proof-coverage.md)). It does **not**
  prove the JSON number conversion, the calculator's rule mix, all Money arithmetic, SQL pool hooks, the schema, external
  providers, the browser or the entire core.
- The documentation build mirrors all tracked Markdown, copies local SVG assets,
  checks links/anchors/publication inventory, and publishes a searchable site after
  a merge to `main`. These diagrams are rendered assets, not unrendered fenced code.

Remaining production work includes complete integer pricing/tax migration, per-tenant
worker/provider token and spend admission, finer runtime/worker privilege separation, full telemetry
export/traces/alerts and independently reviewed public deployment hardening.
