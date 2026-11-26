# Component repositories and release acceptance

This page replaces the November extraction plan that used to live here. That plan
moved domains out of one server crate into separate repositories and services, task by
task (CORE-, CHK-, KNOW-, APP-, CON-, SYS- and others). The work now follows the
[component plan](plans/2026-11-19-component-architecture.md): 22 components inside the
monorepo, each generated into its own mirror repository. The former task list with its
dated evidence is in the Git history of this file; the evidence pages it linked stay in
[`docs/evidence/`](evidence/).

- How the components are built and connected: [component architecture](architecture.md).
- Who owns which file and table: [source map](source-map.md).
- What is released and verified: [status](status.md) and the [release notes](current-release.md).

## Two kinds of separation

**Components inside the commerce server.** The 22 components share one process and one
PostgreSQL transaction. That is deliberate: an order, its stock movement and its outbox
event commit together, and no network call may split that commit. Each component is
still a crate of its own with its own schema baseline, ports and tests, and
`scripts/component_mirrors.py` generates a standalone repository for each of the 19
components that have a crate. `components.release.json` names the target repositories
(for example `veyndo/catalog`, `veyndo/checkout`, `veyndo/app-platform`) and the commit
the mirrors were generated from. Publishing the mirrors to those repositories is not done
yet. [Component mirrors](component-mirrors.md).

**Separately run services.** Some parts run as their own process and talk to the
commerce server only through documented HTTP, MCP or app contracts:

| Service | Contract with the commerce server | Guide |
| --- | --- | --- |
| Connector service | App gateway; own tables and delivery queue | [Connectors](connector-component.md) |
| App runtime and app data service | App contracts, the SDK | [App runtime](app-runtime-service.md), [app data](app-data-service.md) |
| Checkout service | Three commerce host operations (quote, commit, recovery) | [Checkout](checkout-component.md) |
| Knowledge service | Source events and scoped reads | [Knowledge](knowledge-component.md) |
| Automation preview | Scoped rule evaluation over HTTP and MCP | [Automation](automation-component.md) |
| Marketplace and EDI gateways | App records and jobs; orders through orders | [Marketplaces](marketplaces.md), [EDI](edi.md) |
| Payments provider, Storyfront | Generic payment contract; storefront and checkout operations; private repositories | [Payment API](payment-provider-api.md), [Storyfront](storyfront.md) |

The public development repositories are `veyndo/veyndo` (the complete product),
`veyndo/sdk`, `veyndo/apps` and `veyndo/marketplaces`; the
[machine-readable catalog](https://www.veyndo.ai/docs/components.json) lists their
released tags.

## Release acceptance order

Open before the hosted pilot is accepted as a complete release:

1. Match one fresh paid order to its verified capture, Flow, invoice, connector receipt and
   arrival in the reachable test mailbox, in one hosted provider-backed journey through the
   independent Checkout.
2. Measure callback capacity against the configured provider Sandbox before broadening the
   pilot.
3. Accept the component-architecture release on a fresh installation: all registered
   suites on one fresh build, the two-replica purchase and cancellation regression, the
   hot-path benchmark against the starting point and the production image contents (plan
   items 5.5 and 5.6).

Done earlier and recorded with evidence: PayPal Sandbox captures matched to refunds, a
delivered catalogue update read back from Knowledge, scoped Automation keys revoked over
HTTP and MCP, five separate workloads on Northflank with signed database scope, matched
load comparisons of native and independent confirmation, and the Connected Shop example.
[Composed release rehearsal](evidence/composed-release-rehearsal-2026-11-10.md).

The component-architecture release replaces earlier installations. There is no upgrade,
rolling upgrade or data import from an earlier release; a hosted shop is set up on a new
database.

## CORE: modular transactional commerce

The former CORE-02 to CORE-08 tasks (explicit ports instead of broad dependencies, one-way
dependencies, catalog, customer, cart, inventory, order and configuration contracts) are
carried out as the components `catalog`, `customers`, `checkout`, `inventory`, `orders`,
`payments`, `pricing`, `shipping` and `shop`. The transport-free contracts they shared
are described in the [source map](source-map.md#commerce-domain-owners). A standalone
build of each is the component mirror (former CORE-09). The purchase, refund and product
edit journeys now run in the browser (`browser_journeys`); the embedded Storyfront purchase
stays with the release acceptance above (former CORE-10).

## CHECKOUT: independent service and UI

The separate Checkout process orchestrates buyer review and sessions; the checkout
component keeps the one order, stock and outbox commit. Native and Storyfront purchases
share the same quote, commit, payment and recovery contract. Open: the hosted paid-order
chain through invoice and delivered mail, and versioned public HTTP/MCP interfaces for the
Checkout service. [Checkout guide](checkout-component.md).

## Cross-component agentic UI

Studio, the storefront and external hosts compose component views through the SDK layout
contract, MCP Apps and OpenUI; the storefront kit is also published as an MCP App. In the
monorepo every screen lives with its component (`components/<name>/ui`) and imports only
along the declared dependencies. [Agentic component UI](agentic-component-ui.md).

## KNOWLEDGE: first complete standalone AI component

Inside the server, knowledge belongs to the intelligence component. The independent Rust
Knowledge service uses its own PostgreSQL database and OAuth introspection for other
applications. Open: one shared retrieval implementation behind source, storage and model
ports, and switching the product to the released service. [Knowledge guide](knowledge-component.md).

## AUTOMATION and CONNECTORS

Rule evaluation and flow graphs are the `automation` component; the hosted preview runs
the same pinned rule source. The connector service is a separate Rust binary composed of
the connector runtime and one provider crate per bundled app, reached only through the app
gateway. [Automation](automation-component.md) · [Connectors](connector-component.md).

## Acceptance for a separately run service

A separately run service is accepted when it:

- starts from its released image with its own configuration, storage and credentials,
  without the commerce process or its tables;
- runs the same authorized operations over HTTP, MCP and the SDK, with validated schemas
  and error codes;
- enforces current actor, tenant and environment authority, including foreign tenants,
  revoked grants and spoofed headers;
- keeps language inheritance, a non-English main language, a fifth content language,
  channel scope and exact minor units;
- handles revision conflicts, duplicate requests, restart, timeout, lost worker leases and
  ambiguous external effects;
- has unit, contract and real-storage integration tests, and an honest compatibility
  matrix with local, CI and hosted evidence recorded separately.

Backup and restore stay required. Schema migration between releases and rolling
compatibility are not, by the decision of 25 November 2026: a release sets up a new
database.

## Complete-product release acceptance

- Boot a clean composition from the exact released component versions (private services
  only through authorized images and contracts).
- Run signup, roles, catalogue and media, search, direct and embedded checkout, return
  URLs, a localized order, receipt, mail, capture and refund through real browser and API
  paths. Locally covered for the native storefront by `browser_journeys`; the hosted run
  is open.
- Repeat the app lifecycle (create, configure, test privately, publish, update, restore)
  through a browser on the hosted stack.
- POS, marketplace import, B2B/EDI, split shipment and cross-channel stock against the same
  shop and last stock unit: done locally with fixture services.
- Service failure and restart, duplicate events, revoked rights and foreign tenants across
  all storage owners: rehearsed locally and on Northflank; the concurrency suite for
  repeated delivery, lease loss and uncertain sends is open.
- Keep README, website, component references, examples and status in line with the
  released artifacts and the actual evidence.
