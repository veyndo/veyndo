# Read models: plan for the 61 foreign table accesses

Written 27 November 2026 as part of Batch J of the
[component architecture plan](2026-11-19-component-architecture.md). Nothing here is
implemented yet. The plan needs the forward migrations that a parallel Batch J step
is building, because every read model is a new table.

[`components.json`](../../components.json) lists 61 places where one component still
reads or writes a table that belongs to another one. Each place has a written reason
under `foreign_tables`, and `scripts/components.py` refuses new ones. This plan sorts
the 61 into three groups:

| Class | Meaning | Count |
| --- | --- | ---: |
| **a** | An existing event already reports every change the reading component needs. A local read model can be filled from it. If the payload only carries IDs, the projector reads the rest through the owner's port while it projects. | 10 |
| **b** | Some change the reader depends on publishes no event, or the event has no key to route the update. This needs a new event, or new optional fields on an existing one, before a read model can replace the read. | 12 |
| **c** | It has to stay a read or write in the same transaction. The usual reasons are a lock, an atomic claim, an authorization snapshot or an atomic money/stock write. Only the owner moving the statement behind a port changes it. | 39 |

## The event contract this builds on

Read models consume events only through their published contracts. Each outbox kind
has a typed payload in its producer's `types::events` module, declared with
`veyndo_core::event!`. Its JSON Schema is `contracts/events/<kind>.v<N>.json`, and
[`contracts/events/index.json`](../../contracts/events/index.json) lists all 70 kind
versions with their producers and consumers. The gate `scripts/event_contracts.py`
(tooling suite `event_contracts`, quick `verify` job) refuses a breaking change to a
published version. So a projector written against `order.placed.v1` keeps working
until a `v2` appears next to it, and a `v2` payload carries `schemaVersion: 2`.
Additive optional fields stay inside the version. They are also how most (b) items
should get their missing keys.

Rules for every projector:

- **Lane.** Read models are projected in a fifth outbox lane, `models`. It needs a
  new subscriber bit, so the "all lanes done" mask goes from 15 to 31. A forward
  migration sets the bit on rows that are already delivered and adds the
  per-tenant checkpoints. Projection effects and the lane acknowledgement commit in
  one transaction, just like the existing lanes (`components/core/src/store/outbox.rs`).
- **Idempotency.** Each read-model row stores the outbox ID (`source_event`) and the
  owner revision it reflects. A projector applies an event only when it is newer
  than the stored ID. Events of one tenant arrive in ID order within a lane, so a
  replay is a no-op.
- **Backfill.** Each read model gets a rebuild that reads current state through the
  owner's port. The forward migration that creates the table runs it once. After
  that, the lane continues from its checkpoint.
- **Consistency.** A read model trails the owner by one lane pass. Only reports,
  overviews, CRM views and search ranking move to read models. Admission, pricing,
  stock and payment decisions keep their same-transaction reads, which is why they
  are class (c).
- **Ratchet.** Each step deletes its entry from `foreign_tables` in the same commit
  and runs the suites of the touched components on a fresh build.

## Class (a): read models fed by existing events

| # | Reader | Foreign table | Read model in the reader | Fed by |
| ---: | --- | --- | --- | --- |
| 17 | catalog | `b2b_catalog` | `catalog_unit_scopes(tenant, unit_id, product_id, source_event)`. The paginated catalogue, search and category queries join it locally, so cursors stay correct. | `b2b.catalog_changed`, then the unit's list through the b2b port; `shop.release.published` for released units. Purchase admission (`b2b::store::catalog::admit_unit`) stays synchronous, so a stale scope can show a product but never sell one outside the unit. |
| 27 | customers | `carts` | `customer_order_digests(tenant, order_id, customer_id, customer_email, cart_email, state, payment_state, total_minor, currency, placed_at, revision, source_event)` | `order.placed`, then the order and its cart email through the orders port; `order.state_changed`, `payment.state_changed`, `delivery.state_changed`, `order.updated`, `payment.updated`, `payment.captured` |
| 28 | customers | `orders` | the same `customer_order_digests` | the same |
| 37 | marketing | `commerce_overrides` | `report_channel_overrides(tenant, channel_id, data, revision, source_event)` | `commerce.configured` (the `ChannelConfigured` variant carries `channelId`), then the override through the shop port; `shop.release.published` |
| 44 | intelligence | `commerce_settings` | `intelligence_search_weights(tenant, weights, revision, source_event)`. The ranked retrieval statement joins it locally. | `commerce.configured` (every variant), `shop.release.published` |
| 47 | intelligence | `orders` | `intelligence_order_facts(tenant, order_id, channel_id, currency, total_minor, state, payment_state, placed_at, simulated, revision, source_event)` | `order.placed` and `migration.record.imported` (kind `orders`, marked historical), then the order through the orders port; the three `*.state_changed` kinds; `payment.captured`, `payment.updated` |
| 48 | intelligence | `payment_attempts` | the columns `attempt_id, provider, environment, real_money_charged, attempt_state` of `intelligence_order_facts` | `payment.captured`, `payment.updated`. Their payload already carries attempt, state, provider, environment and `realMoneyCharged`. |
| 55 | studio | `carts` | `report_orders(tenant, order_id, channel_id, attribution, state, payment_state, refund_state, simulated, total_minor, currency, guest_email, placed_at, revision, source_event)` | `order.placed`, then the cart attribution through the checkout port; the state and payment kinds |
| 58 | studio | `orders` | the same `report_orders` (guest contacts and aggregates) | the same; `migration.record.imported` (kind `orders`) for historical orders |
| 59 | studio | `payment_attempts` | `refund_state`, `simulated` in `report_orders` | `payment.updated` (refund states), `payment.captured` |

Flow actions change order tags, custom fields and automation data through
`orders::store::rule_orders` without an event. None of these read models may carry
those fields. Order state changes from flows go through `merchant.order.transition`
and publish their normal transition events.

## Class (b): new events or fields first

| # | Reader | Foreign table | What is missing | Event to add | Read model |
| ---: | --- | --- | --- | --- | --- |
| 18 | catalog | `knowledge_documents` | Claim creation and relation writes (`intelligence::store::relations`, staging copies) publish nothing. `intelligence.claim.reviewed` has no product or claim value. | `intelligence.claim.changed` v1 (claimId, productId, documentId, state, visibility, revision) from every claim write; document visibility already has `knowledge.document.*` | `category_fact_claims(tenant, product_id, claim_id, facet, value, locale, public, source_event)` |
| 19 | catalog | `knowledge_relations` | same | same | same |
| 30 | orders | `order_downloads` | The flow action `action.grant.download.access` (`catalog::store::rule_products`) grants without an event. Grants at placement are covered by `order.placed`. | `product.download_granted` v1 (orderId, productId, assetId, expiresAt) | `order_download_view(tenant, order_id, product_id, asset_id, expires_at, source_event)` |
| 31 | orders | `product_assets` | Asset uploads, media jobs and staging copies publish nothing | `product.asset.changed` v1 (productId, assetId, revision, deleted) | `order_asset_meta(tenant, asset_id, product_id, filename, mime, size, revision, source_event)` |
| 41 | intelligence | `app_evidence` | Purging an app's private sources (`apps::store::evidence`) publishes nothing | `app.evidence_purged` v1 (app, removed) | `intelligence_evidence_inventory(tenant, app, sources, active, revision, source_event)` |
| 42 | intelligence | `app_packages` | Install and uninstall come from vendored app storage (`app.installed`, external). Deactivation publishes nothing; `app.activated` only covers hosted frontends. | `app.deactivated` v1, and an `app.activated` variant for every activation path in `components/apps` | same table, column `active` |
| 45 | intelligence | `customers` | Registration, profile and password (`customers::store::accounts`), flow group/status changes (`rule_customers`), restore and provisioning publish nothing | `customer.changed` v1 (customerId, email, group, active, revision) from one owner hook used by every customers writer | `intelligence_customers(tenant, customer_id, email, group_id, active, revision, source_event)` |
| 50 | intelligence | `product_reviews` | Moderation (`catalog::store::reviews`) publishes nothing; imports only publish `migration.record.imported` | `product.review.moderated` v1 (reviewId, productId, state, revision) | `intelligence_review_counts(tenant, product_id, published, revision, source_event)` |
| 51 | intelligence | `products` | `product.created`, `product.updated` and `product.published` come from studio, translations, test stock and reverts. Categories, stock, rule products, staging and demo templates in catalog write without one. | `product.changed` v1 (productId, revision, contentRevision) from one catalog hook in `catalog::store::products`. It replaces the synchronous `knowledge::sync_product` calls in the producers. The price revert becomes a catalog port call that keeps its revision check. | the existing knowledge product projection, filled by the lane instead of in the writer's transaction |
| 56 | studio | `customers` | as for #45 | `customer.changed` | `directory_contacts(tenant, email, customer_id, registered, name, group_id, orders, last_order_at, revision, source_event)`, also fed by `order.placed` for guest contacts, so the keyset page and the app predicates run on one local table |
| 57 | studio | `merchant_users` | Account creation and name changes (`identity::store::merchant_accounts`, `broker`, provisioning) publish nothing | `merchant.account.changed` v1 (userId, displayName, revision) | `history_actors(tenant, user_id, display_name, source_event)` |
| 60 | studio | `sales_channels` | Channel saves (`marketing::store::configuration_writes`, staging copies) publish nothing | `sales_channel.changed` v1 (channelId, kind, active, revision) | `kind` column on `report_orders`; the same event can later replace the operator channel count in shop |

The new kinds are declared with `event!` in the owner's `types::events`, published as
v1 with `scripts/event_contracts.py --publish`, and added to the Flow catalogue only if
merchants should be able to trigger flows from them.

## Class (c): stays a same-transaction access

| # | Component | Table | Why it stays |
| ---: | --- | --- | --- |
| 1–7 | core | `app_deliveries`, `app_schedule_runs`, `app_webhook_receipts`, `flow_jobs`, `knowledge_receipts`, `observed_pairs`, `pair_evidence` | Retention retires or deletes outbox payloads in the same `SKIP LOCKED` statement that proves no row references them. A separate read could race with a new foreign-key reference. |
| 8 | identity | `app_packages` | The credential snapshot admits an integration key only while its package is active with the same digest. A lagging copy would accept a revoked package. |
| 9 | identity | `sales_channels` | The request channel is part of the same authoritative credential snapshot. |
| 10 | identity | `shop_environments` | A stage or preview resolves to its live shop inside the credential snapshot, the agent authority and MFA. |
| 11 | identity | `tenants` | A suspended shop must stop authenticating immediately. Workspace lists use the same join. |
| 12 | shop | `company_logos` | A write: sandbox and release copy logos with their settings in the staging transaction. |
| 13 | shop | `company_overrides` | A write in the staging transaction; channel deletion counts them under the same lock. |
| 14 | shop | `sales_channels` | Request admission reads the channel with shop status and settings in one statement. The operator count could later use `sales_channel.changed` (#60). |
| 15 | shop | `shop_environments` | The request snapshot scopes a stage or preview to its live shop. |
| 16 | shop | `tenants` | The operator lifecycle locks and changes the shop status next to the sandbox check. |
| 20 | catalog | `languages` | A static global registry with no runtime writer, so there is no event to feed a copy. It belongs in core as shared reference data, joined where it is read. |
| 21 | catalog | `shop_environments` | The media job claim admits only active live shops inside its `SKIP LOCKED` claim. |
| 22 | catalog | `tenants` | same claim |
| 23 | checkout | `customer_sessions` | It binds a buyer request to its account (authentication) in one statement. |
| 24 | checkout | `customers` | Customer group and company set prices and admission inside the purchase. A stale copy could price with an old group. |
| 25 | checkout | `shop_environments` | Sandbox and preview scoping of buyer requests. |
| 26 | checkout | `tenants` | Shop status and the agent stop switch inside the claim or join. |
| 29 | orders | `carts` | A write: order mutations mirror the order into the cart snapshot the buyer reads, under the order lock. The real fix is buyer reads through `orders::http::account_orders`, which deletes the mirror. That is not a read model. |
| 32 | orders | `sales_channels` | Admits the request channel for company data reads. |
| 33 | payments | `app_packages` | App-provided payment providers and actions must be active in the transaction that uses them. |
| 34 | payments | `carts` | A write: payment state on the cart under its lock. |
| 35 | payments | `orders` | A write: the ledger and the order payment state change atomically. |
| 36 | marketing | `app_packages` | The configuration digest has to cover the exact installed revisions in the statement that reads rules, flows and promotions. Otherwise a sandbox release could compare a stale digest. |
| 38 | marketing | `tasks` | The flow-template proposal checks its task in one statement with the job lease. |
| 39 | apps | `shop_environments` | Delivery and schedule claims exclude staging inside one `SKIP LOCKED` statement. |
| 40 | apps | `tenants` | The delivery claim locks the owning shop row (`FOR UPDATE OF d,owner SKIP LOCKED`). |
| 43 | intelligence | `channel_metrics` | Traffic counters change on every request. One event per increment would flood the outbox. The counters stay shared core data. |
| 46 | intelligence | `exposures` | A write, and the wrong owner: experiment exposures belong to intelligence. Checkout's reward count already goes through a port (`count_purchase`). Move the table rather than copy it. |
| 49 | intelligence | `privacy_consents` | A write: consent receipts are the legal gate, and behaviour reads must see the current receipt in the same statement. Move ownership. Do not copy. |
| 52 | intelligence | `shop_environments` | Admission, claim and work statements resolve the live shop. |
| 53 | intelligence | `tenants` | Embedding and vector claims skip inactive shops inside the claim. |
| 54 | intelligence | `translation_jobs` | A write: agent runs cancel the translation jobs they started, atomically with the run state. It becomes a catalog port call. |
| 61 | studio | `shop_environments` | The sandbox control plane (list, count, baseline lock, preview checks) needs the current row. Writes already go through `veyndo_core::store::environments_write`. |

Several (c) entries are writes or claims that are better removed by moving
ownership or adding a port than by projecting: #29, #46, #49, #54 and #20. They are
listed so that the next step does not try to turn them into read models.

## Migration order

All of this waits for the forward-migration framework. Then:

1. **Infrastructure.** Add the `models` lane, its forward migration (subscriber bit,
   checkpoints, mask 31), a shared projector helper with the `source_event` guard, a
   rebuild hook per read model and a suite that replays the outbox twice and
   compares. No foreign table changes yet.
2. **Class (a), lowest risk first.** Each item is one commit with its table,
   backfill, projector and removed `foreign_tables` entry:
   1. \#44 `intelligence_search_weights`
   2. \#37 `report_channel_overrides`
   3. \#47, #48 `intelligence_order_facts`
   4. \#55, #58, #59 `report_orders` (without channel kind until #60)
   5. \#27, #28 `customer_order_digests`
   6. \#17 `catalog_unit_scopes`. It is last because it affects catalogue
      visibility. Keep `admit_unit` synchronous.
3. **Class (b).** First the event in the owner, published as v1. Then the read
   model:
   1. \#60 `sales_channel.changed` (also completes `report_orders`)
   2. \#57 `merchant.account.changed`
   3. \#50 `product.review.moderated`
   4. \#41, #42 app evidence and package lifecycle
   5. \#30, #31 download grants and asset metadata
   6. \#45, #56 `customer.changed`
   7. \#18, #19 `intelligence.claim.changed`
   8. \#51 `product.changed`. It is last because it replaces the synchronous
      `sync_product` calls in several writers.
4. **Class (c) follow-ups, not read models.** Move `exposures` and
   `privacy_consents` to intelligence, `languages` to core, buyer order reads to
   orders, and translation job cancellation behind a catalog port.

When steps 2 and 3 are done, `foreign_tables` drops from 61 to 39 entries. Every
remaining entry is a (c) reason from the table above.
