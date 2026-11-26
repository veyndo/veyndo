# Source responsibilities and verification map

This map names the owner of each area and the checks that cover it. It follows the
[component architecture](architecture.md): 22 components declared in `components.json`,
each with its Rust crate, schema baseline and screens. Two generated pages hold the
complete lists: the [module inventory](module-inventory.md) names every source file and
the [dependency report](component-dependencies.md) counts dependencies, foreign table
accesses and the ratchets. When this page and `components.json` disagree,
`components.json` is right and this page is stale.

To find the owner of a file, look it up in `components.json` (`sources`); to find the owner
of a table, look in `tables`. `python3 scripts/components.py` checks both.

## Component owners

| Component | Crate and entry points | Owns these tables and rules | Main checks |
| --- | --- | --- | --- |
| core | `components/core` (`veyndo-core`), `crates/kernel`, `crates/runtime` | Tenants, outbox, leases, read-context versions, AI usage and resource limits; request context, database scope (`components/core/src/store/{pool,row_security}.rs`, `crates/runtime/src/scoped_pool.rs`), admission, typed document views (`components/core/src/document.rs`), the verified kernel | `production_foundations`, `transaction_pooler`, `store_isolation`, formal suites |
| identity | `components/identity` (`veyndo-identity`) | Merchant accounts, memberships, sessions, MFA, invitations, roles, integration keys, agent authority; `components/identity/src/route_policy.rs` | account security and tenant isolation suites |
| shop | `components/shop` (`veyndo-shop`) | Shop settings and their revisions, domains, languages, legal texts, operator control | international commerce, settings and channel suites |
| catalog | `components/catalog` (`veyndo-catalog`) | Products, categories, media, reviews, translations, product files, catalogue reads | catalog, product management and translation suites |
| inventory | `components/inventory` (`veyndo-inventory`) | Stock reservations and supplier links; `InventoryLines` | `production_foundations`, property and mutation tests |
| pricing | `components/pricing` (`veyndo-pricing`) | Money (`components/pricing/src/domain/money.rs`), price calculation, discounts, currencies, taxes, customer groups, price jobs | `currencies`, `international_commerce` and `formal_differential` suites, property and mutation tests |
| shipping | `components/shipping` (`veyndo-shipping`) | Delivery methods, destinations (`components/shipping/src/domain/geography.rs`), delivery splits, carrier booking | `shipping_contract`, `shipment_workflows` suites |
| checkout | `components/checkout` (`veyndo-checkout`) | Carts, quotes and the purchase transaction (`components/checkout/src/service/purchase/{lock,price,commit}.rs`) | checkout and handoff suites, property and mutation tests |
| customers | `components/customers` (`veyndo-customers`) | Customer accounts, sessions, addresses, the merchant directory | account and CRM suites |
| orders | `components/orders` (`veyndo-orders`) | Orders, the order state machine (`components/orders/src/domain/order_workflow.rs`), receipts, documents, downloads, company order data | order workflow suites, property and mutation tests |
| payments | `components/payments` (`veyndo-payments`) | Payment ledger, attempts, captures, refunds, provider accounts and workers (`components/payments/src/service/worker.rs`) | payment provider fixtures, property and mutation tests |
| marketing | `components/marketing` (`veyndo-marketing`) | Rules, coupons, promotions, flows and flow jobs, sales channels | automation and channel suites, promotion properties |
| b2b | `components/b2b` (`veyndo-b2b`) | Companies, units, budgets, approvals, quotes, shopping lists | B2B and UCP B2B suites |
| pos | `components/pos` (`veyndo-pos`) | Register adapters over checkout, orders and payments; no tables | POS suite |
| apps | `components/apps` (`veyndo-app-platform`), `crates/apps` | App packages, managed records, settings, jobs, deliveries, webhooks, surfaces, Wasm extensions | app platform and app storage suites |
| automation | `crates/automation` (vendored, hash-locked), `crates/flow-graph` | Rule evaluation and flow graphs; no tables | `automation`, `automation_lifecycle` suites |
| inference | `crates/cognition` | Model provider adapters, budgets, prompt provenance, image generation; no commerce writes | inference unit tests |
| intelligence | `components/intelligence` (`veyndo-intelligence`) | Knowledge, shop memory, agents and runs, proposals, consent, personalization, catalogue intake | knowledge, agent and behaviour suites |
| studio | `components/studio` (`veyndo-studio`) | Overview, reports, staging shops, history, manual orders, flow work templates, catalogue translation worker | staging, report and history suites |
| protocols | `src/mcp`, `src/ucp`, `src/capabilities` | MCP, UCP and capability dispatch; no tables, no business rule | `schema_contracts`, MCP and UCP suites |
| host | `src/` (everything else), `crates/port-contracts` | Process entry, route registry, worker roles, schema setup, port implementations, provisioning | the HTTP suites, `schema_baseline` |
| connectors | `extensions/services/connectors`, `extensions/services/connector-runtime`, `extensions/apps/<app>` | Six `connector_*` tables, the provider delivery queue | `rust_connectors`, `email_tests` |

## Commerce domain owners

The same component operations serve the storefront, Studio, the API, MCP, UCP, POS and
agents. An adapter or app translates inputs and outputs; it never becomes a second owner
of a price, an order or stock.

| Domain | Owner | Invariants to keep |
| --- | --- | --- |
| Catalog, categories, variants and media | catalog (`components/catalog/src/{store,service,http}`); the server keeps the product edit handler that also calls legal, marketing, knowledge and the app host (`src/commerce/product_admin.rs`) | Tenant and channel filtering, translation inheritance, content revision, variant identity, redaction of internal facts |
| Customers, accounts and addresses | customers (`components/customers`), sign-in through its `SignIn` port; storefront account routes in `src/accounts/` | Verified account identity, immutable order ownership, current PII permissions |
| Cart and checkout | checkout (`components/checkout`); cart routes and storage in `src/cart_routes.rs`, `src/cart_storage.rs` | Cart revision, legal and payment review, idempotency, recovery of the original receipt, one commit transaction |
| Pricing and taxes | pricing (`components/pricing`), the checkout cart projection (`components/checkout/src/service/cart_projection.rs`); tax configuration through shop settings | Currency scale and rounding, authoritative rule facts, price snapshots, tax context; FX only in pricing (see [currencies](currencies.md)) |
| Inventory | inventory (`components/inventory/src/store/reservations.rs`) on the checkout transaction (`components/checkout/src/service/purchase/commit.rs`) | Ordered product locks, aggregate stock revisions, one allocation per purchase, idempotent release |
| Orders | orders (`components/orders`); imported orders insert through orders (`components/orders/src/store/imports.rs`); manual orders in studio | Immutable amounts and buyer snapshots, tenant identity, state guards, atomic order events |
| Payments and refunds | payments (`components/payments`); the server's `src/payments/` implements the payments host port and provider routes | Verified amounts, allocations, receipt uniqueness, current authority, uncertainty after interrupted provider calls |
| Fulfilment and documents | shipping (`components/shipping/src/domain/split.rs`), orders (`components/orders/src/http/delivery_split.rs`), server document routes in `src/operations/{order_documents,receipts,delivery_booking}.rs` | Persisted order quantities, split conservation, original receipt identity, provider callback evidence |
| Settings and international context | shop (`components/shop/src/types/settings.rs`), the server's settings guards (`src/commerce/settings_guards.rs`) behind `veyndo_shop::ports::SettingsGuards` | Current grants, basis and channel inheritance, revisions, dependent method usage |

## Shared customer input contract

`veyndo_pricing::customer` owns `Contact` and `Address` with their bounded syntax
validation. Account, administration and checkout code reuse the same types; native
adapters map the portable validation error to the HTTP or MCP error. The type has no
database, network or identity dependency, and constructing it grants no permission:
PII projection, login and immutable order ownership stay with customers and orders.
The [standalone example](../components/pricing/examples/customer_contract.rs) runs without
a server: `cargo run -p veyndo-pricing --example customer_contract`.
[Package guide](../components/pricing/README.md).

## Shared cart contract

`veyndo_pricing::cart` owns the cart item and the persisted cart data contract. Its context
parameters are bound to the POS, B2B, app and checkout definitions in
`components/shop/src/types/cart.rs`; those keep their serializers and validators. The stored
cart (tenant identity, bearer token, status, revision and hydration) belongs to checkout.
Building the DTO grants no authority: identity, grants, review, pricing and commit
admission stay in the checkout operations.

## Shared product contract

`veyndo_pricing::catalog` owns the product snapshot and tier DTO used by catalog, cart,
rules and order paths. It depends on Serde and the pricing types only. Catalog hydrates the
same type through its tenant-scoped reads (`components/catalog/src/store/products.rs`;
the server's `src/catalog_model.rs` composes it for the routes). Public product
serialization removes the internal automation facts, including supplier purchase prices,
while the server snapshot keeps them.

## Inventory write boundary

`veyndo_inventory::InventoryLines` pairs product IDs with positive quantities that fit the
signed database representation, and it binds the Lean-proved per-order bound
(`reservation_admissible`). Native checkout (including POS) and imported orders build the
batch once; the same batch drives the allocation ledger and the stock decrement. Tenant
scope, availability checks, ordered locks and release from the persisted ledger stay in
inventory. `production_foundations` and the marketplace suites cover order and
cancellation behaviour.

## Server host modules

`src/` belongs to `host`, except the protocol adapters. It holds routes, port
implementations and composition, not business rules; each module's first line names its
responsibility.

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Module declarations and `main` only |
| `src/foundation.rs` | `App`: composition of component-owned state |
| `src/routes.rs`, `src/routes/http_profile.rs` | Route registry; the API-only profile admits only protocol routes ([hosted component API](hosted-component-api.md)) |
| `src/ports/` | The server's implementations of component ports, one file per declaring component |
| `src/workers.rs`, `src/workers/` | Independently deployable worker roles with leases and durable receipts |
| `src/migrations.rs`, `src/migrations/schema.rs` | Fresh-installation setup with the ordered component baselines |
| `src/store/` | The server's own SQL: shop provisioning across components, the operator control plane, process checks |
| `src/auth/`, `src/accounts/` | Middleware, registration, provisioning and broker flows not yet moved to identity; storefront account routes |
| `src/apps/`, `src/developer/` | Host ports of the app platform: callback dispatch, gateway transport, job import, site builder rules; prompt-generated declarative apps |
| `src/commerce/`, `src/operations/` | Catalogue and checkout routes that compose several components; merchant CRM and fulfilment APIs shared with MCP |
| `src/payments/`, `src/pos/`, `src/b2b/`, `src/marketing/` | Route and host-port layers over the payments, POS, B2B and marketing components |
| `src/cognition/`, `src/documents/`, `src/agent_runs/`, `src/translations/` | Server execution of runs, ingestion and translation over the intelligence and catalog components |
| `src/platform/`, `src/staging/`, `src/history/`, `src/studio/` | Operator control plane; route registration for staging shops and history owned by studio |
| `src/performance/`, `src/outbox.rs`, `src/outbox/` | Read-context caching with authoritative versions; outbox projection and retention workers |
| `src/bin/` | `connectors` entry and the verified kernel driver (`verified_kernel/`) |

## Protocol adapters

`src/mcp.rs` and `src/mcp/` hold the typed MCP schemas and JSON-RPC transport, `src/ucp/`
the UCP adapters, `src/capabilities.rs` and `src/capabilities/` the shared HTTP/MCP capability
dispatch with tool authorization. They call the component operations. `scripts/mcp_stdio.py`
is an optional client-side bridge; the hosted `/mcp` server is Rust.

## Frontend

| Path | Responsibility |
| --- | --- |
| `frontend/src/main.tsx` | Entry |
| `frontend/src/admin/`, `frontend/src/storefront/`, `frontend/src/platform/`, `frontend/src/application/` | Studio and storefront shells, the operator console, application wiring |
| `frontend/src/shared/` | Shared building blocks (grids, history, canvas, i18n, geography) |
| `frontend/src/kit/` | Design tokens, UI parts, storefront kit, generated API client (`kit/api/generated`), query layer |
| `components/<name>/ui/admin/<area>`, `components/<name>/ui/storefront/<area>` | Screens of 13 components |
| `frontend/scripts/source-roots.mjs` | Gives the architecture, localization and translation gates one logical path set for both roots |

Rules: [frontend/AGENTS.md](../frontend/AGENTS.md); localization:
[localization guide](localization.md).

## Connector service and apps

| Path | Responsibility |
| --- | --- |
| `src/bin/connectors.rs` | Process entry only |
| `extensions/services/connectors/src/` | Service: HTTP authentication, OAuth completion, actions, workers |
| `extensions/services/connector-runtime/src/` | Encrypted store, queue, leases, pinned egress, OAuth state, exports |
| `extensions/apps/<app>/src/` | One `veyndo-provider-<app>` crate per bundled provider (email, Gmail, Google Analytics, Slack, Algolia, Contentful, HubSpot, Klaviyo, Vercel) |
| `extensions/services/connectors/migrations/0001-baseline.sql` | The six connector tables with forced row security |
| `reference/connectors-python/` | Archived comparison implementation for the differential fixtures only |

Details: [Rust services](rust-services.md), [connectors](connector-component.md),
[email delivery](email-delivery.md).

## Schema, fixtures and tooling

| Path | Responsibility |
| --- | --- |
| `components/core/migrations/0000-foundation.sql` | Extensions, `veyndo_authority`, shared trigger, history and scope functions |
| `components/<name>/migrations/0001-baseline.sql` | One baseline per component; creates only its own tables |
| `fixtures/demo/` | Demo data, applied after the schema |
| `fixtures/schema/reference.sql` | Reference schema of a fresh installation for `schema_baseline` |
| `.sqlx/` | Committed checked-query cache; `scripts/sqlx_cache.py` regenerates it |
| `scripts/components.py` | Ownership, dependency, schema-order and ratchet check; writes the dependency report |
| `scripts/component_mirrors.py` | Component mirror generation and check |
| `scripts/structure.py` | The required quick structure check |
| `scripts/verify_integration.py`, `scripts/testing/suites.json` | Suite runner and suite registry |
| `proof/` | Lean model, manifest and domain inventory |

## Verification map

| Area | Suites and checks |
| --- | --- |
| Tenant isolation | `store_isolation`, `production_foundations`, `transaction_pooler` ([tenant isolation](tenant-isolation.md)) |
| Schema | `schema_baseline`, `sql_prepare` |
| Domain rules | `cargo test` per component (unit, `properties`, `domain_mutants`), `scripts/testing/mutation_domain.py`, `formal_differential` |
| Interfaces | `schema_contracts`, port contracts in `crates/port-contracts`, `testing/component_mirrors_check` |
| User journeys | `browser_journeys` |
| Coverage | `scripts/testing/coverage_report.py` with per-component floors |

The [testing guide](testing.md) explains each suite; the
[formal verification guide](formal-verification.md) the proofs.
