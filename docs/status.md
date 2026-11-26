# Current status: what is live and tested

This page says what the current release is, what was verified for it, what runs in
the hosted pilot and what is still open. It is kept short on purpose: the dated
observations of earlier releases are in the [changelog](../CHANGELOG.md), in
[`docs/evidence/`](evidence/) and in Git history. The [release notes](current-release.md)
describe what changed.

| You want to know | Go to |
| --- | --- |
| The idea and where to begin | [Start here](start-here.md) |
| How the parts fit together | [Component architecture](architecture.md) · [One shop, composable parts](one-shop-components.md) |
| What changed in this release | [Release notes](current-release.md) |
| Which page owns a topic | [Handbook index](README.md) |

Updated 26 November 2026.

## Release state

The **component architecture** release is a release candidate on branch
`refactor/component-architecture`. It has not gone live yet. It is a fresh installation
only: it replaces earlier installations, there is no upgrade path or data import, and
setup refuses a database that an earlier release created
([what operators must know](current-release.md#what-operators-must-know)).

## Platform at a glance

- **One core, 22 components.** `components.json` declares 22 components with their
  sources, tables and allowed dependencies; 19 have crates of their own. The server
  composes them and holds the HTTP, MCP and UCP adapters. No two components depend on
  each other; the required structure check refuses a new cycle, an undeclared
  dependency or an unexplained foreign table access
  ([architecture](architecture.md), [dependency report](component-dependencies.md)).
- **Checked queries and one schema per component.** 1,267 queries are checked at
  compile time from the committed `.sqlx` cache. Each component owns one baseline schema;
  `schema_baseline` proves that a fresh installation produces the expected schema
  ([schema baselines](rust-services.md#schema-baselines)).
- **Exact money.** Prices, totals and taxes run in whole minor units; the price
  calculator computes with exact decimals and rounds each amount once
  ([multi-currency commerce](currencies.md)).
- **Proofs.** 74 commerce rules with 217 properties are proved in Lean and bound to the
  Rust functions that use them; every commerce `domain/` function is either bound to a
  proved rule or listed as unproved with a reason ([proof coverage](proof-coverage.md),
  [domain inventory](formal-coverage.md)).
- **Tests.** 116 registered suites and tools in `scripts/testing/suites.json`, plus
  property tests, store isolation under forced row security, port contract tests,
  generated HTTP/MCP contract cases, mutation testing of `domain/`, per-component
  coverage floors and browser journeys ([testing](testing.md)).
- **Mirrors.** Each of the 19 components with a crate has a standalone mirror that is
  checked to build and test on its own ([component mirrors](component-mirrors.md)).
- **No Shopware compatibility.** Shopware remains one of the import sources of the
  [Migration Assistant](agentic-migration.md); no Shopware code runs or is compared.

## Verified for this release candidate

- The required structure check: ownership of every source file and table, the
  dependency direction, the schema order and the falling ratchets.
- The suites of each area the restructuring touched, on a fresh build in that round;
  `schema_baseline`, `store_isolation`, `schema_contracts`, `formal_differential` and
  `testing/component_mirrors_check` when they were added; the property tests and the
  mutation run of 25 November.
- The money and exact-decimal changes with the `commerce`, `checkout_handoff`,
  `checkout_review`, `payments`, `payment_providers`, `international_commerce`,
  `currencies`, `agentic_commerce`, `b2b`, `pos`, `production_foundations` and
  `formal_differential` suites on a fresh build.

## Open before go-live

- All registered suites in one run on a fresh build of the merged branch, the
  two-replica purchase and cancellation regression and the hot-path benchmark against
  the starting point.
- The production images checked by their contents and startup path.
- The first CI run of `browser_journeys` and the first nightly run with the
  per-component coverage floors.
- A fresh hosted installation of this release; the hosted pilot below still runs the
  earlier release.

## Hosted pilot (earlier release)

Until the go-live, the hosted Lumen pilot on Northflank runs the **earlier release**.
Its recorded journeys keep their dates and scope and are not evidence for this
release. The main records:

| Area | Record |
| --- | --- |
| Separately hosted services composed into one shop, recovery and load | [Composed release rehearsal, 10 November](evidence/composed-release-rehearsal-2026-11-10.md) |
| Roadmap stages 1 to 3, proof coverage, sovereign AI, trade network, signed offers, experiments, autopilot, shop twin, attack test | [Stages 1–3](evidence/stages-1-3-completion-2026-11-18.md) and the linked records of 18 November |
| PayPal Sandbox purchases and refunds through Shopware Payments | [Provider acceptance](shopware-payments-acceptance.md) |
| Separately hosted components and app storage | [Component hosting, 5 November](evidence/component-hosting-2026-11-05.md), [app storage](evidence/app-storage-component-2026-11-05.md) |
| Merchant reports, canvas, assistant sidebar | [Reports](evidence/merchant-reports-2026-11-03.md), [canvas](evidence/canvas-workspace-2026-11-02.md), [sidebar](evidence/persistent-copilot-2026-11-03.md) |
| Connected commerce and agentic purchases | [Connected commerce, 30 October](evidence/connected-commerce-2026-10-30.md), [agentic commerce](evidence/agentic-commerce-hosted-2026-10-18.md) |
| Customer scenarios and merchant journeys | [Scenarios](evidence/customer-scenarios-2026-10-22.md), [merchant journey](evidence/merchant-journey-2026-10-22.md) |
| Product Guide in the shopper and merchant conversations | [Product Guide, 11 November](evidence/product-guide-hosted-2026-11-11.md) |
| Current stock in the original Storyfront | [Storyfront stock, 11 November](evidence/storyfront-stock-hosted-2026-11-11.md) |
| Earlier releases up to 21 October | [Release history](evidence/release-history-through-2026-10-21.md) |

## Status by area

| Area | Owner guide | Known limits |
| --- | --- | --- |
| Catalogue, stock and categories | [Product management](product-management.md) | Bounded category navigation; no product streams or CMS layouts |
| Cart, checkout and orders | [Checkout](checkout.md), [merchant operations](merchant-operations.md) | One payment transaction per order |
| Payments | [Payments](payments.md), [provider API](payment-provider-api.md) | Real PSP acceptance only for the recorded Sandbox journeys; no recurring payments |
| Currencies, taxes and countries | [Currencies](currencies.md), [international commerce](international-commerce.md) | Merchant-entered tax configuration; no external tax services |
| B2B, EDI, POS, marketplaces | [B2B](b2b-commerce.md), [EDI](edi.md), [POS](pos.md), [marketplaces](marketplaces.md) | Live seller accounts and EDI networks are not certified |
| Rules and flows | [Rules and flows](automation.md) | 108 of 114 catalogued condition types are bound |
| Apps | [App platform](app-platform.md), [App Studio](app-studio.md) | Arbitrary source builds run outside the core |
| Intelligence and agents | [How the intelligence layer works](cognitive-commerce.md), [AI workspace](agentic-workspace.md) | Model quality and causal uplift are not claimed |
| Security and tenancy | [Security](security.md), [tenant isolation](tenant-isolation.md) | Database administrators keep system authority |
| Operations | [Production architecture](production-architecture.md), [deployment](deployment.md), [cloud releases](cloud-releases.md) | No automatic backup, restore or failover |

## Start testing

Run Veyndo locally with the [quickstart](quickstart.md), take the
[first tour](first-tour.md) or let an agent buy with [agent purchase](agent-purchase.md).
