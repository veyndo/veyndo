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
- [x] Lower `too-many-lines-threshold` from 320 to 150. The 25 longer functions
      of our own code are split into named steps without a change in behaviour
      (route registry, capability schema, sandbox release and clone, POS
      gateway, B2B quotes, authentication middleware, external orders, job
      import, order transition, EDI export, proposal apply, app callbacks,
      delivery enrichment, merchant operations, promotions, migration source
      pages, manual orders, mailbox sync, inference request, flow mutations,
      browser session, checkout commit, scenario journey and probes, receipt
      text). The hash-locked vendored app storage keeps its former limit through
      `extensions/services/clippy.toml`, which covers only the crates below it.
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
- [x] Move request admission into `core`. The settings and language read caches
      belong to `shop` and move with it in phase 3; `components.json` assigns them so.
- [ ] Replace the `App` state object by per-component state plus ports. Done so
      far: `veyndo_core::State` (database, HTTP client, admission) that the server
      state dereferences to; the outbox `Projector` port; the validated
      process configuration (`core::process_config`), which the server builds
      and installs once at startup and every component reads.
      The app platform has its own state (`veyndo_app_platform::ports::Platform`:
      the shared state, its execution limits and the host ports), built by the
      server from `App`. Its ports are `RecordRules` (the record rules of the
      bundled site builder), `CommerceInstall` (configuration lock, flow
      replacement check, payment methods) and `HostActions` (the action
      handlers that touch carts, files, payments or search).
- [x] Remove the dependencies `core` has on other components (`uses` of core in
      `components.json` is empty). Server wiring (`App`, outbox routes) is assigned
      to the host; crate-to-crate dependencies are read from crate names.
- [x] `components/identity` (`veyndo-identity`): permission vocabulary and decision
      under `domain/`, `permit` and `merchant` as the checks other crates call;
      the proved scope rule is bound there.
- [ ] Move the rest of `src/auth` (accounts, sessions, MFA, invitations, route
      policy) into `components/identity` as `types/`, `store/`, `service/`, `http/`.
      Started: `store/roles` holds the grants of a role, the rehydrated current
      actor and the usage of a role; `http/members` the workspace member list
      and the role or revocation change, `http/access` the access overview,
      the personal session list and the revocation of sessions and invitations;
      `store/handoffs` the one-time Studio handoff ticket; `http/browser` the
      cookie transport of the personal session (host-only cookies, exact
      origin, CSRF) as request middleware. `store/sessions`
      issues personal sessions and MFA challenges, `http/sessions` owns login,
      logout and workspace discovery, `http/invitations` the invitations,
      `http/mfa` enrollment, confirmation, disabling and verification,
      `store/credentials` the one credential lookup (`identity.sql`),
      `http/integrations` the integration keys, `store/broker` the signed
      broker assertion with its nonce and `http/broker` password setup,
      recovery discovery and the broker session; `types/envelopes` the typed
      request shapes. Open: the broker exchange and inference (they provision
      a shop and call the model), registration and provisioning, browser
      sessions, the middleware with login throttling, the Knowledge
      introspection, the role editor (it validates names against shop settings).
- [x] Ban new `use super::*` / `use crate::*`: `glob_imports` per component in
      `components.json` only falls (528 files at the start); component crates have none.

Done when the server builds on both crates without glob imports from them.

## Phase 2: pilot `pricing`

- [x] `crates/commerce` is `components/pricing` (`veyndo-pricing`) with its rules
      under `domain/`. The `service` feature (default) adds what needs `core`;
      without it the crate is the pure domain, which the migration app uses.
- [x] Currency configuration (`types/currency`), conversion and reference-rate
      retrieval (`service/`) moved into `components/pricing` with their tests.
- [x] First ports (`ShopSettings`, `Carts` in `ports.rs`), implemented by the
      server; currency discovery and the price job reads are `http/` and `store/`
      of the component, with typed rows and tests against test ports.
- [x] Price job creation (through a locked-settings port), tax classes with the
      destination rate, customer groups and the context selection rules moved.
- [x] Move the price job worker, the rate refresh and the removal guard. The
      worker (`service/price_job_worker`) locks products, writes fixed prices and
      marks the history through the `Catalogue` port; the server keeps only the
      timer loop. Retrieving rates is pricing, storing them in the shop settings
      stays with `shop`, as do tax and group validation, because they write or
      check the whole settings object; currency selection on a cart belongs to
      checkout.
- [ ] Money is one type in minor units everywhere; the column "uses floating
      point" on the proof coverage page is zero for this component. Done so far:
      cart totals (goods, tax, payable, net) are added as minor units by proved
      rules. The calculators themselves stay a bit-exact floating-point port of the
      original ones; replacing them needs the PHP comparison run, which CI has
      and the local machine does not.
- [ ] Typed DTOs at the interface; `sqlx::query_as!` with offline cache in `store/`.
      So far rows are typed with `query_as` and checked by `sql_prepare.py`; the
      macro needs the `macros` feature and a prepared-query cache in CI.
- [x] Write down what the pilot cost and what the template needs changed:
  - Pure rules and data types move in one step each. Everything that takes the
    server state does not: 12 of the 13 remaining currency functions call
    `commerce::scoped_config` (shop settings) or `load_cart`/`cart_json` (checkout).
  - Template change 1: a component declares what it reads from others in
    `ports.rs` as a trait; the server implements it. First ports: `ShopSettings`
    (settings and revision for a shop and channel) and `Carts` (load, quote).
  - Template change 2: a component router is generic over the server state and
    takes `veyndo_core::State` by `FromRef`; `secure_route` moves to identity.
  - Template change 3: a crate feature `service` separates the pure `domain/`
    from the parts that need `core`, so independent apps keep a small dependency.
  - Template change 4: test files in a component name their imports; the
    component check refuses `use super::*` at file level.

## Phase 3: the remaining components

Order follows the dependency report, lowest first. Each line is one move.

- [ ] catalog. Started: `components/catalog` owns
  - product rows: hydration, the locked range read and fixed-price write that
    price jobs use (the server's `Catalogue` port delegates to it), the
    template-shop read;
  - the commerce fields of a product (typed payload, bounds, save that keeps an
    untouched stock), media rules, GLB admission and the model asset check;
  - reviews: submission, moderation and import as `http/`, `service/` and
    `store/`, behind the ports `Shoppers` (cart, purchases) and `Shop` (preview
    environment, history);
  - the product edit payload with its pure checks (translations, metadata, image
    texts, which clock an edit is checked against) and every SQL statement of an
    edit in `store/product_edit`; identity, channel visibility and category
    assignment as a service;
  - categories: the edit with its checks, the saved fact query rule, the tree
    read and the serialized cycle-safe write as `types/`, `store/`, `service/`
    and `http/`; the shop's languages come through the `Shop` port;
  - the public navigation (visitor resolved through the `Shop` port, hidden
    branches as a pure rule) and the category seed of a new shop;
  - translated product hydration as a pure rule over typed rows, with the
    share-locked read checkout uses; the merchant list query (closed sort,
    cursor, escaped search, a hook for app filters) and the preparation view;
  - product files: content admission, the multipart parser, upload, list and
    publish;
  - rich text and the content fallback rules.
  - translations: which texts of a product are translatable, admission of a
    model response, the explicit product selection, and applying reviewed
    drafts (at most 50 per request, a stale product becomes a conflict). The
    share-locked shop languages come through the `Shop` port; the
    `revision_admissible` binding now names the catalog service.
  - image jobs: the request rule and the model instruction, admission (one per
    product, ten pending per shop), the queue, the private preview and the
    revision-bound publication. The image model is the port `Images`; the
    server's implementation holds the shop's model lease around the call.
  The content language rules (`Languages`: key, allowed, translated names) and
  the sort direction went to `core`, because other components use them too.
  Open: the handlers of product edit, list and creation still run in the server,
  because they also call legal, marketing, knowledge, the app host and the shop
  settings. Also open: paid downloads, caption proposals (they drive
  inference), the translation
  worker and job routes (they drive inference), storefront pages, staging of
  categories, demo data. `domain/product_admission` admits a product write as
  a full or an explicit category-only edit and builds the assignment-only
  edit against the clock the merchant read; `domain/csv_rows` is the bounded
  CSV reader of the catalogue intake.
- [ ] inventory. Started: `components/inventory` owns the checked quantity batch
  (`InventoryLines`, moved out of pricing), the allocation ledger with the
  idempotent release, stock consumption, supplier links with their bounds, the
  rule that admits a signed supplier offer (saved link, pricing currency,
  ceiling), and test stock as a service behind the ports `Environments`
  (private sandbox) and `ProductIndex`. Open: the reorder proposal and its
  approval still run in the server, because they call the supplier shop's
  cart, signed offers and the checkout handoff; reservations of open carts
  (`inventory_reservations`); `query_as!`.
- [ ] shipping. Started: `components/shipping` owns the bundled world with the
  shop's own countries and the address region rule, the split of a package
  (quantities conserved, no second shipping charge), the carrier booking
  transition, the reference counts of delivery and payment methods with their
  preflight handler, and the calendar delivery window. Both kernel bindings
  (`delivery_split_admissible`, `carrier_booking_admissible`) name the new
  files. Translated method texts (`pick`, `validate_texts`) went to `core`.
  Open: shipping cost and tax enrichment of a quote (it reads the shop
  settings, cart and currency types), the split and booking handlers (they
  write through the order mutation log), delivery method types.
- [ ] customers. Started: `components/customers` owns the address book (typed
  statements, the bounded revision-checked save and delete, defaults that are
  never left empty), customer sessions (only the token digest is stored), the
  typed customer row, the contact projection, order metrics, the bounded
  lookup with literal search, the guest contact and the remembered payment
  method. Open: the handlers still run in the server, because they resolve the
  storefront request, attribute the history and read shop settings; login;
  the merchant customer list and detail (they attach app
  extensions). `store/crm` owns the merchant creation of a contact with its
  address plan and the revision-checked merchant change including the
  restoration of a historical state. Registration, the
  contact profile and the password swap are writes in `store/accounts`; the
  handlers keep credential hashing, geography and channel admission. The restoration of an address aggregate from history
  lives in `store/`; the shop's geography admits each address through a
  closure. `http/browser` is the customer cookie transport in front of the
  tenant session ledger; `http/crm_context` gives CRM writers their creation
  choices.
- [ ] payments. Started: `components/payments` owns the attempt type, the
  payment state machine, wire amounts at ISO precision, receipt identity and
  references, settlement and refund budgets, recipient instructions, adapter
  service admission, the details model, the receipt guard, public diagnostics,
  return URLs and the method catalogue in `domain/`. `store/` holds the row
  mapping, receipt claims, job admission, saved instruments, the order ledger
  (order update, events, stock release) and the capture, settlement and refund
  admissions; `service/` the provider-neutral receipt persistence and funding
  admission, the adapter request, the leased job worker, signed provider
  notifications, embedded provider sessions with their parent origin check,
  transaction details, recipient instructions and app payment commands; the
  command queue with refund admission and the customer binding sit in
  `store/`. `ports::Host` is what the server supplies to the ledger: adapter
  configuration and the POS sale completion. `ports::Gateway` is the provider
  side: the admitted adapter transport, provider dispatch, receipt
  persistence per provider, the worker scope and the built-in provider's
  cancellation evidence. The proved payment rules are bound to the new files.
  `service/provider_configuration` validates the provider origin, accounts
  and adapter services once at startup.
  Open: provider accounts and onboarding, the registry of methods from app
  manifests, routes, the PayPal adapter include.
- [ ] checkout. Started: `components/checkout` owns the destination tax lines
  of a quote, the shipping and payment method enrichment with delivery dates,
  and exact app price adjustments. `domain/` also holds the binding of an
  order to the reviewed cart revision and total, the reviewed session
  purchase with its replay and receipt checks, and the classification of the
  checkout test payment; `store/` the localized SKU snapshot read under the
  purchase locks, the channel admission of a session and its placed order,
  the immutable customer and address record of an order, and the legal
  acknowledgement and consent checked under the purchase locks and frozen
  into the order (`legal_checkout_admissible` names the new file).
  The kernel bindings of the review name the new file. The cart quote with
  quantity pricing, configuration fees and minor-unit totals is
  `domain/cart_quote`; the language chain of a stored cart is
  `service/language`. Open: cart storage and mutation, the selection, the four steps of the checkout
  transaction (they call apps, B2B, payments, marketing and legal and need a
  host port), the handoff. `http/buyer_identity` seals and inspects the
  short-lived Checkout grant of a live customer session and
  `http/private_browser` relays the two browser commands to the private
  Checkout service; the server only registers their routes.
  `domain/admin_order` and `store/admin_order` hold the guards of an order a
  merchant places (manual or simulated payment only, channel policy, the
  channel's product selection); `store/selected_customer` rechecks the
  selected customer under a shared lock for checkout and the POS.
- [ ] orders. Started: `components/orders` owns the order state machine with
  its default and bounds, the stored machine, derived order fields, the
  transition guards and the workflow view, the audited write log with its
  replay ledger, and receipt texts. The kernel bindings of the order workflow
  name the new file. `store/` also holds the redacted read views for app
  callbacks, the merchant order list (admitted app filters join through a
  closure), the order detail with its activity, notes, the company basis with
  its channel overrides and public projection, company logos, the issuance
  of invoices, delivery notes and cancellations, and carrier documents.
  `ports::Payments` supplies the transfer instructions an invoice prints.
  The orders and receipts an owning customer account reads live there too,
  as does the selective company release of a staging environment.
  Open: the grid plan, manual orders, imports, the workflow save (app
  lifecycle), fulfilment handlers, returns, merchant customer edits.
- [ ] marketing. Started: `components/marketing` owns the rule conditions with
  their bounds and matching, field comparison, the line, customer and order
  facts a rule reads, the snapshot of referenced rules, and the shared
  condition service. `types/` holds the promotion with its import provenance,
  the flow, the work template and the sales channel with its POS location;
  `domain/` the flow action schema, the flow graph and instruction texts;
  `store/` promotion evaluation with usage limits, channel validation and the
  dependency inspection before a deletion, the flow job claims with their
  checkpoints, the membership a queued step runs with and the bounded job
  summaries and the revision-bound write of rule metadata (its bounds are in
  `domain/metadata`; `domain/condition_import` translates source conditions;
  `domain/rule_comparison` and `store/rule_history` compare a saved rule with a
  draft on frozen Flow inputs; `store/flow_mutations` applies a local flow
  action to its order or customer once per flow key;
  `domain/configuration_kinds`, `store/configuration_lifecycle` and
  `http/configuration` hold the right and table of each configuration kind,
  the tenant lock, the revision-bound deletion with its dependency read and
  the admission of rule references); `service/` the coupon selection of a cart, which admits and
  prices it through `veyndo_shop::ports::Carts`. The outbox flow projection
  is `domain/flow_projection` and `store/flow_projection`; sales channel
  admission (marketplace, cashier and preview rules, served locales, product
  selection, company catalog) is `domain/channel_admission` and
  `service/channels`. Open: flow execution (needs ports to orders, customers
  and apps), channel preview and access tokens, rule history, routes.
- [ ] b2b. Started: `components/b2b` owns the company, unit and member records
  with their bounds and the capability contract; `store/` holds unit access
  and the cart guard, organization reads and writes, budgets and approval
  thresholds of a purchase, the private unit catalog and the staging copy;
  `service/` the approval inbox, quick order and the accepted offer contract.
  They reach the buyer cart through `veyndo_shop::ports::Carts`, which the
  server implements with its guarded cart load, priced view and
  revision-checked line replacement, and `admit` runs the cashier, channel
  and company guards on a cart row the caller locked itself. The requisition
  import lives in `service/`; the bounds of cart lines moved to
  `veyndo_shop::domain::cart_lines`. Open: lists (app configuration
  selections), quotes and their lifecycle (app lifecycle), the gateway and
  routes.
- [ ] pos. Started: `components/pos` owns the cashier input envelope and the
  capability schema; the POS location of a channel lives with the channel in
  marketing. `service/` holds the cashier customer search and attachment,
  the audited cashier discount in quote and commit, cash admission, the order
  annotation and the sale completion; `store/` the customer rows a cashier
  sees. They price through `veyndo_shop::ports::Carts`. The cart admission
  every adapter runs (merchant-created cart, cashier, sales channel) moved to
  `veyndo_shop::domain::cart_admission`. The kernel binding of cash
  admission names the new file. Open: the gateway (it dispatches into
  catalog, currencies, coupons, legal and the checkout) and the channel
  installation (it validates settings through the rule engine).
- [ ] apps (with `developer` and the vendored app storage). Started:
  `components/apps` (crate `veyndo-app-platform`; `crates/apps` stays the
  sandbox without SQL or network authority). It owns
  - `domain/`: package approval against bundled and operator-pinned digests,
    publisher signatures and namespaces, the field, reference, view and
    manifest policy with the package compiler adapter, configuration inputs,
    budget, saved selections and the receipt snapshot, record lifecycle, the
    graph mapping, core extension rules, consent, action admission and
    authorization, entity access, callback grants, discovery rules, the event
    admission, privacy projection, batch size, destination and signature, the
    webhook signature and shop binding, secret kinds, the schedule grammar, the
    service transport policy and UI address, and the native error mapping;
  - `domain/` also holds native lifecycles with their compiler adapter and
    the assistant view rules (tool admission, view address, attachments and
    the view document);
  - `store/`: the record store, managed reads and writes, record restoration,
    package readers and the package write fence, installation and activation
    with their hooks, the dependency gate, schema upgrade, language and
    reference checks, secrets and declared settings, schedules, event
    deliveries, the action log, configuration events, app filters, durable
    jobs with retry admission, the storage budget, private evidence reads,
    lifecycle capture, commerce hook admission, the native-object projection,
    assistant view lookup and the surface registry;
  - `service/`: pinned egress, local limits and network admission, the module
    compilation cache, the configuration executor with its frozen examples,
    operator service calls, event delivery, package verification, the upgrade
    review, record import, job claim and progress callbacks with sealed
    completion evidence, service job requests and the action gateway;
  - `domain/` further holds the surface grant rules, app planning, the App
    Studio generation rules (schema, guided brief, bounded edits, retained
    contract, deterministic bindings, design context, default actions) and
    the build admission;
  - `store/` further holds surface grants, planning, the tool catalogue, cart
    configuration checks, the purchase projection and hosted app
    installation;
  - `service/` further holds saved selections, App Studio builds (save, list,
    archive, staging), the immediate preview and capability discovery;
  - `http/`: secrets, settings, activity and replay, callback keys, incoming
    webhooks, jobs, record history, surface grants, UI bundles, declared app
    routes, the action and entity handlers, App Studio autosaves, archive,
    staging and preview, capability discovery, package activation and the
    consent adapter; the server registers them with their route rights;
  - `ports`: the `Platform` state and the three host ports named in phase 1.
    `HostActions` also creates the personal preview environment.
  The operator's services and publisher keys reach pure rules as parameters.
  The tool wire contract lives in `veyndo_core::tool_contract`.
  Open (about 3,000 lines remain in `src/apps`, 360 in `src/developer`): core
  callbacks, documents and exchange (they call operations, assets and B2B),
  job import, evidence ingest (knowledge sync), cart contributions and
  commerce hook evaluation (cart and currencies), submissions (cart and site
  forms), asset surfaces, the package list and installation route (shop
  settings, hosted frontends), the site builder, and in `developer` the
  generation orchestrator and coding-agent task export (inference and design
  references stay with the server).
- [ ] automation
- [ ] intelligence. Started: `components/intelligence` owns the knowledge
  module (relations, hybrid search, vectors with their namespace and cache,
  embeddings, reranking), the bounded behaviour event and experiment model,
  knowledge sources and their published documents (`domain/source_content`,
  `store/documents`), PDF text extraction in a bounded child process
  (`service/pdf`), the embedding generations (`store/embedding_generations`),
  controlled experiments with their readout and worker steps
  (`store/experiments`), behaviour observations with the channel report and
  the private journey (`store/behavior`), the private preference graph
  (`domain/preference_graph`, `store/preferences`) and the signed offer
  projection (`domain/offer`), the claim ledger with claim batches
  (`store/evidence`, `domain/claim_batch`), the order observation projection
  and published pairs (`store/observations`), the embedding job queue
  (`store/embedding_jobs`), source search, the source inventory and product
  facts (`store/documents`) and the bounded prompt context
  (`domain/prompt_context`, `store/context_products`) and durable agent
  runs in storage: the caller projection, the conversation turn fence,
  lease-fenced publication and the checkpoint journal (`store/agent_runs`,
  `domain/run_result`); `domain/trusted_geography` accepts a visitor
  country only from the trusted ingress. The server keeps
  consent, settings, channel admission, provider admission and leases and
  passes them in.
  The claim lifecycle and public claim batches (`service/claims`,
  `http/claims`), the knowledge workspace (`http/workspace`), experiment
  operations (`service/experiments`) and source content checks
  (`service/sources`) read settings through the shop read cache.
  Open: agents and their runs, proposals, the retrieval and question
  handlers, inference, cognition routes and workers.
- [ ] studio. Started: `components/studio` owns report configuration and
  calendar periods, the bounded personal workspace model
  (`domain/workspace`, with the parts of a composed record view in
  `domain/view_components`) and its revisioned store (`store/workspaces`),
  the overview facts with their turnover totals (`store/overview`) and the
  work list (`store/work_items`) and the report query on a read snapshot
  with its traffic read (`store/reports`; the server passes settings, the
  reader's language and the behaviour consent filter) and the release units
  of a staging shop for knowledge sources, assets and categories
  (`store/staging_documents`, `store/staging_assets`,
  `store/staging_categories`), and the reading side of history: which
  entities keep versions, their rights, the operation schemas and the
  restoration request (`domain/history_access`), the product edit rebuilt
  from a version (`domain/product_restore`), version reads (`store/history`)
  and the listing and version handlers (`http/history`), and the report
  service with its handler (`service/reports`, `http/reports`). Open: the rest of
  staging shops (clone, snapshot, release), the history restore dispatch (it
  replays edits through roles, B2B, catalog, customers, settings, marketing
  and documents and needs their ports), the channel activity tracker.
- [ ] shop. Started: `components/shop` owns the settings types (`Settings`,
  delivery and payment methods, the checkout selection, B2B switches, AI
  guardrails, legal configuration), every bound on them (`validation`, tax
  rules, customer groups), the bundled method labels and the sparse channel
  override. It sits above pricing, inventory and shipping, whose types it
  embeds; rule conditions are checked through a function the rule engine
  passes in. It also owns the cart as every component reads it (`Cart`,
  `StoredCart`, the cashier discount, the company context, app
  configurations), the admission of a request to a cart
  (`domain/cart_admission`), the bounds of cart lines and the cart port
  (`ports::Carts`: guarded load, priced view, admission of a locked row,
  revision-checked line replacement). `store/settings` reads the settings of
  a channel under a share lock. The operator control plane has started to move: the per-shop AI
  quota, aggregate metrics, the shop dossier and the audited lifecycle change
  (`http/operator_*`) and the container readings
  (`service/container_resources`); operator authentication and availability
  admission stay in the server. `store/consumer_requests` owns the withdrawal
  and data-rights intake, the receipt of its holder and the operator review.
  `http/operator_session` answers the operator's own session and the
  control-plane audit trail.
  The process-wide read cache of settings and the language registry, with
  its per-request memo and the startup registration of the condition check,
  is `service/read_cache`; the request language and its translation chain
  are `domain/language_selection` and `service/language`.
  Open: saving and releasing settings, channel scope handlers,
  domains, legal consent, the rest of the platform (AI settings,
  infrastructure, provisioning) and the performance modules.

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

- [x] Unit tests for every function in `domain/`. Each of the 216 public domain
      functions of the components is exercised by a test in its own file; private
      helpers are reached through them.
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
      is never negative. Proved and bound in cart pricing: addition (`amount_sum`,
      four properties), net plus tax equals gross and net never above gross
      (`net_amount`), discount within its base (`discount_cap`). Open: scaling
      and rounding.
- [ ] Price calculation: the order total equals the sum of its lines, shipping and
      tax for every input; the same input gives the same quote.
- [ ] Stock: a reservation never takes stock below zero; reserve followed by
      release restores the quantity; an order holds at most what it ordered.
      Proved: the first two, also for two reservations in sequence
      (`stock_admissible`, five properties). Open: the per-order bound.
- [x] Checkout: an order is placed only for the reviewed cart revision and total
      (`checkout_review_exact`); a repeated request places no second order
      (`replay_exact`, `replay_same_cart`, `replay_open_fingerprint`).
- [x] Payments: captured never exceeds authorised (`capture_bounded`), refunded
      never exceeds captured (`refund_bounded`, also in sequence), and every ledger
      transition is one of the listed ones (`payment_transition_exact`).
- [ ] Orders: every reachable state of the order machine is a listed state, and
      completed or cancelled orders accept no further fulfilment.
- [x] Access: a credential of one shop never satisfies a check for another
      (`merchant_scope_admissible`, three properties, bound in the identity
      crate); a missing permission never admits (`scope_exact`).
- [x] Signed offers: an expired offer or a changed cart, revision or total is never
      honoured (`offer_honoured`, four properties).
- [x] Sovereign AI mode admits only the self-operated provider on a self-hosted
      endpoint (`sovereign_request_admissible`, four properties).
- [ ] Extend the extractor to saturating addition and multiplication together with
      the first rule that uses them. Addition is done (`amount_sum`); multiplication
      follows with line totals. The price calculation is still a floating-point
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
