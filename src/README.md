# src

This folder owns the Rust modules listed below. Each source begins with its responsibility contract. The crate currently shares internal types/imports through a facade; APIs, MCP and UCP delegate to shared domain operations.

- [`agent.rs`](agent.rs): Persistent grounded conversations and tenant-scoped semantic knowledge HTTP adapters.
- [`bootstrap.rs`](bootstrap.rs): Startup, additive migrations, persisted extensions and outbox worker.
- [`capabilities.rs`](capabilities.rs): Shared HTTP/MCP capability dispatch and tool authorization.
- [`cart_mutation.rs`](cart_mutation.rs): Optimistic cart mutations and quantity normalization.
- [`cart_price.rs`](cart_price.rs): Authoritative quantity pricing and localized checkout quote assembly.
- [`cart_routes.rs`](cart_routes.rs): Store API cart and order route adapters.
- [`cart_storage.rs`](cart_storage.rs): Cart creation, loading and input validation.
- [`catalog_model.rs`](catalog_model.rs): Tenant product model and database hydration.
- [`catalog_page.rs`](catalog_page.rs): Bounded tenant-scoped catalog reads. A cursor is a product ID, never an offset.
- [`catalog_routes.rs`](catalog_routes.rs): Health and localized catalogue HTTP routes.
- [`channel_metrics.rs`](../components/core/src/channel_metrics.rs): Bounded, lossy diagnostic counters. Never use this buffer for business events.
- [`agent_runs.rs`](agent_runs.rs): Existing-task continuation, private checkpoints and lease-fenced native proposal completion.
- [`checkout_handoff.rs`](checkout_handoff.rs): Single-use checkout transfer for independent storefronts; no app-specific catalog or checkout rules.
- [`concierge.rs`](concierge.rs): Read-only storefront shopping advisor.
- [`customer.rs`](customer.rs): Customer credential verification and context rotation.
- [`discount.rs`](../components/pricing/src/domain/discount.rs): Integer-cent proportional discount allocation; cumulative rounding conserves the exact basket discount.
- [`experience.rs`](experience.rs): Persisted storefront layout policy and observed synthetic rewards.
- [`extensions.rs`](extensions.rs): Merchant catalogue and Wasm extension activation/state.
- [`foundation.rs`](foundation.rs): Application dependencies, error responses and request context helpers.
- [`http_limits.rs`](../components/core/src/http_limits.rs): Bounded streaming responses for extension services and payment providers.
- [`inference.rs`](../crates/cognition/src/inference.rs): Provider adapters. Credentials stay on the server; domain validation is separate.
- [`knowledge.rs`](../components/intelligence/src/knowledge.rs): Transactional PostgreSQL knowledge relations and separately indexed Qdrant retrieval.
- [`lib.rs`](lib.rs): Reusable pricing, context, sandbox, graph and inference modules.
- [`localization.rs`](localization.rs): Shop locale resolution, translated catalog hydration and non-mutating merchant quote.
- [`main.rs`](main.rs): Process lifetime only. See docs/source-map.md for domain responsibilities.
- [`mcp.rs`](mcp.rs): Typed MCP schemas and JSON-RPC transport.
- [`migrations.rs`](migrations.rs): Versioned setup is separate from serving; no catalog-wide startup repair.
- [`order_checkout.rs`](order_checkout.rs): Atomic checkout, stock locks, extension policy and idempotency.
- [`order_routes.rs`](order_routes.rs): Merchant order read adapter.
- [`outbox.rs`](outbox.rs): Durable outbox and audit projection worker.
- [`planner.rs`](planner.rs): Grounded model planning, recorded inputs and proposed changes.
- [`pricing.rs`](../components/pricing/src/domain/pricing.rs): Behavioral port of Shopware 6.7.14.2 quantity calculators.
- [`proposal_apply.rs`](proposal_apply.rs): Transactional application of approved, revision-bound proposals.
- [`proposal_routes.rs`](proposal_routes.rs): HTTP proposal creation, approval and task listing.
- [`routes.rs`](routes.rs): HTTP transport registry; domain behavior lives in dedicated modules.
- [`rule_comparison.rs`](../crates/automation/src/rule_comparison.rs): Behavioral port of Shopware 6.7.14.2 RuleComparison::numeric and FloatComparator's exact epsilon boundaries.
- [`sandbox.rs`](../crates/apps/src/sandbox.rs): Pure Wasmtime guest execution with bounded resources and no host imports.
- [`seed.rs`](seed.rs): Idempotent synthetic template catalogue initialization.
- [`shop_domains.rs`](shop_domains.rs): Resolve configured shop subdomains before authentication; reject unknown hosts and conflicting scopes.
- [`studio.rs`](studio.rs): Verified merchant overview facts consumed by the chat and activity views.
- [`ucp/`](ucp/): Versioned UCP checkout, discovery and native B2B negotiation adapters.
- [`verified_kernel.rs`](../crates/kernel/src/verified_kernel.rs): Closed, side-effect-free commerce policies extracted to Lean; keep within the checked bool/u64 grammar.
- [`workers.rs`](workers.rs): Independently deployable worker roles; leases and durable receipts coordinate replicas.

The [source inventory](../docs/module-inventory.md) is checked in CI. [The behavioral map](../docs/source-map.md) identifies integration suites, and [testing](../docs/testing.md) describes actual coverage and limits. Every file is limited to 320 lines; `main.rs` to 120.

## Open JSON

Server files read and write stored documents through typed views (`veyndo_core::document::view`,
lenient fields) and typed patches (`document::merge`); no `value["key"]` access remains outside
tests. The following values stay `serde_json::Value` because their shape is open by contract;
the typed views carry them through unchanged:

- `history/restore.rs` `Snapshot.data`: an audited row snapshot of any entity; each entity's own
  save handler validates it on replay.
- `apps/data.rs` `SiteRecord.fields`, `apps/app_flows.rs` `InputSchema.properties` and the event
  an app action receives: app-declared data and schemas, validated by the app platform.
- `apps/job_import.rs` `ImportInput.record` / `options`: the source record and approved options of
  an import job, admitted by the native import operations.
- `developer/generation.rs` and `developer/routes.rs` `manifest`: model output or an agent's app
  definition, repaired and validated by the app generation contract before it is saved.
- `marketing/flow_actions.rs` order action arguments: the merchant's stored action configuration,
  validated by the flow action schema; the flow adds the typed `OrderActionArgs`.
- Passthrough parts of projections (`studio.rs` overview facts, `checkout/session_quote.rs`,
  `cart_mutation.rs` and `catalog_routes.rs` quote parts, `catalog_routes.rs` product elements):
  documents owned and shaped by another component, forwarded without interpretation.
- `bin/` conformance and comparison drivers: test-case values handed to the policy under test.
