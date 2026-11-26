# Pre-go-live UX acceptance: 26 November 2026

Local acceptance of the release candidate (branch `claude/r4-ux-c1b6e3`, based on
`bfb5d2d3` of `refactor/component-architecture`) in a real browser. Every workflow
below was walked by hand in the built-in browser; registered integration suites were
run for the flows that need fixtures a fresh shop does not have (POS, B2B, provider
payments). Screenshots are in [ux-2026-11-26/](ux-2026-11-26/) (WebP, about 320 KB
in total).

## Setup

- **Fresh install only.** A new PostgreSQL database in container
  `veyndo-roadmap-postgres-1`, shared local Qdrant `http://127.0.0.1:16333`,
  `BOOTSTRAP_MODE=migrate` then serve, on `127.0.0.1:8797`. No upgrade, no data import.
- `cargo build --workspace --bins`; frontend built into `frontend/dist` with Node 24 in
  Docker (`npm ci && npm run build`). The server reads `frontend/dist` from disk, so
  frontend fixes were rebuilt and re-checked in the same session; Rust fixes were
  rebuilt and the server restarted.
- Shop and owner created with `scripts/dev_account.py` (shop `ux-accept`, fashion demo
  catalogue: 12 products, 34 SKUs). A synthetic storefront customer was registered
  through the storefront. Both passwords were generated locally into the ignored
  `.run/` folder and filled into the forms by a one-shot loopback helper; no password
  appears in this record or in the session.
- No model service: `OLLAMA_URL` pointed at a closed port, so assistant behaviour
  without a model was checked. No paid model calls, no external providers.

## Workflows

Viewports: desktop 1440×900, mobile 375×812. Themes: light and dark (Studio has its
own theme switch; the storefront deliberately stays light under a dark system scheme).

| # | Workflow | Languages / viewports / theme | Result | Issue → fix commit |
| --- | --- | --- | --- | --- |
| 1 | Storefront home, hero, collection teaser, consent banner (decline optional) | DE desktop light, EN desktop, DE mobile | Pass after fix | Every page view sent a 404 probe to the uninstalled GA4 app (console error) → `a5cdb590` |
| 2 | Category navigation (Jacken & Mäntel) | DE desktop | Pass | — |
| 3 | Product search, including no results | DE desktop | Pass after fix | Empty grid without any message → `cac4f25a` |
| 4 | Product page with colour/size variants | DE + EN desktop, DE mobile | Pass after fix | Sizes offered as M, L, S → `f3f0bd5d` |
| 5 | Add to cart, quantity, remove | DE + EN desktop | Pass after fix | Line read "Harbor Wollmantel · L · camel / L" (fixture variant names repeated the size) → `49989248` |
| 6 | Coupon: unknown code, then a merchant-created code | DE (unknown), EN (WELCOME10) | Pass after fixes | Summary showed "Subtotal 107.10 / welcome10 −11.90 / Total 107.10" → `3351f2ce`; coupon call with an expired cart token answered 500 → `24e0fdeb` |
| 7 | Guest checkout: address, country, shipping and payment choice, review, simulated payment | DE desktop, EN mobile | Pass after fixes | Standard shipping shown at 4.90 while the 249 € cart charged 0.00 (free from 150) without a hint → `a1c1450c`; delivery window as raw ISO dates → `5ec5e04d` |
| 8 | Order confirmation | DE desktop | Pass after fix | ISO delivery dates (same fix `5ec5e04d`) |
| 9 | Customer sign-up | DE desktop | Pass | — |
| 10 | Customer sign-out and sign-in (returns to the last account view) | DE desktop | Pass | Consent banner is asked again after sign-up and sign-out: consent is deliberately not carried across identity changes (server `continue_consent`) |
| 11 | Account addresses: add, default billing/shipping | DE desktop | Pass | — |
| 12 | Signed-in checkout with the saved default address | DE desktop, EN desktop dark scheme | Pass after fix | An address added after sign-in left checkout on "Neue Adresse" with empty fields → `3feebcee` |
| 13 | Account orders list and order detail | DE desktop | Pass | — |
| 14 | Legal pages: imprint, privacy, terms, withdrawal form | DE desktop, ES + DE mobile | Pass (texts unpublished on a fresh shop, as designed) | Imprint shows only a heading, see open items |
| 15 | Language switch in the storefront header | DE/EN desktop and mobile | Pass after fix | Picker clipped to "Deutsch (Deutsc" and listed "Englisch (Vereinigtes Königreich)" → `c0942e47` |
| 16 | B2B entry in the storefront account | DE desktop | Pass (company access must be enabled by the merchant) | Full flow by suite `b2b`, `b2b_ucp` |
| 17 | Studio sign-in | DE desktop | Pass | — |
| 18 | Studio navigation, all 21 areas (workspace, setup, assistant, shop today/reports, knowledge, agent commerce, products, orders, customers, POS, design, storyfronts, sales channels, rules & flows, apps, App Studio, staging & releases, team & access, settings, companies, B2B quotes): no failed request, no alert, no console error | DE desktop light; EN desktop dark; EN mobile | Pass | Text heuristic for untranslated English on DE pages: only merchant content in the main language (en-GB), which is the content-language contract |
| 19 | Dark theme contrast heuristic (text vs background < 3:1) over the 21 areas | EN desktop dark | Pass with one nit | "Business" chip on My workspace at 2.7:1 |
| 20 | Studio at 375 px: horizontal overflow over the 21 areas | EN mobile dark | Pass | Tables scroll inside their cards |
| 21 | Product edit: price with German decimal comma, stock | DE desktop | Pass | Stored 165.50 / 28, revision 2 |
| 22 | Product translation via the content-language picker | ES content, DE UI | Pass | Storefront returns ES/DE/EN names independently |
| 23 | Product create while the editor shows a non-main language | DE content in an EN-main shop | Pass after fix | Generic "check required fields…" from a 400 for the empty main name → `327fcd81` |
| 24 | Orders: list, detail, start processing, capture payment, ship with tracking | DE desktop, EN dark list | Pass | Completion stays disabled until delivery is confirmed, with a hint |
| 25 | Refund | — | Not walked in the browser | Simulated demo payments have no refund action by design; provider refunds are covered by suite `browser_journeys`, not runnable here (see open items) |
| 26 | Promotions: create coupon (percentage, rule "all orders", active) and redeem it | DE UI, EN content; EN storefront | Pass | Same fixes as #6 |
| 27 | Settings: company, legal & privacy, languages, countries, currencies, tax, shipping methods, payment methods, customer groups, company purchasing, sales channels | EN desktop | Pass (load and render; no mutation) | — |
| 28 | Studio theme switch | DE/EN | Pass after fix | German label "Ansicht wechseln" (switch view) → `75932cb8` |
| 29 | Apps: install a bundled app with permission consent | EN desktop dark | Pass after fix | The consent dialog pushed **Install** out of view behind a horizontal scroll → `6caff8ee` |
| 30 | Assistant without a model service | EN desktop dark | Pass (clear message, resumable run) | Warning wording, see open items |
| 31 | POS register | EN desktop dark (empty state) | Pass | Sale, cash, receipt, roles by suite `pos` |
| 32 | MCP: `tools/list` (39 public tools), `catalog.search`, `cart.create`, `cart.replace`, `cart.quote` as in [agent purchase](../agent-purchase.md), against the local shop | — | Pass | — |
| 33 | UCP: `/.well-known/ucp` discovery and `POST /ucp/v1/carts` | — | Pass (200) | — |

## Registered suites run on this branch

Fresh `cargo build --workspace --bins`, isolated databases, `scripts/verify_integration.py
--container veyndo-roadmap-postgres-1 --keep-going`:

| Batch | Suites | Result |
| --- | --- | --- |
| Flows behind the browser walk | `protocols`, `checkout_review`, `marketing_accounts`, `customer_accounts`, `pos`, `b2b`, `b2b_ucp`, `fashion_demo` | All pass (65 s) |
| Studio areas checked by loading only | `studio`, `merchant_reports`, `users`, `workspace_roles`, `apps`, `staging`, `automation`, `catalog_management`, `app_studio`, `legal_privacy`, `currencies`, `app_consent`, `translations`, `durable_agents` | All pass (165 s) |

Both batches end with `scripts/structure.py`, which passes.

Frontend: every changed or added unit test file passes under vitest (Node 24 in Docker),
plus `tsc --noEmit`, `npm run architecture`, `npm run localization` and Prettier on the
touched files. Rust: `cargo test -p veyndo-marketing --lib`, the fashion fixture test in
`veyndo`. `scripts/structure.py` passes after `source_inventory.py --write`; the proof
manifest review for the changed marketing file was recorded with `scripts/formal.py
--generate --record-review`.

## Fixes (13 commits)

| Commit | Change | Regression test |
| --- | --- | --- |
| `a5cdb590` | GA4 storefront client probes only when the registry lists the app | `frontend/tests/unit/analytics-sandbox.test.tsx` |
| `cac4f25a` | Collection empty state with "clear search"; no flash before the first page | `frontend/tests/unit/storefront-collection-empty.test.tsx` |
| `f3f0bd5d` | Variant sizes in natural order | `frontend/tests/unit/variant-option-order.test.ts` |
| `49989248` | Fashion fixture variant names without the size | `demo_catalog::tests` in `src/demo_catalog.rs` |
| `a1c1450c` | "Free from …" on shipping methods with a threshold | `frontend/tests/unit/checkout-methods-free-shipping.test.tsx` |
| `5ec5e04d` | Localized delivery window (kit `day`) in cart summary and confirmation | `frontend/tests/unit/kit-storefront.test.tsx` |
| `3feebcee` | Checkout adopts default addresses added after sign-in | `frontend/tests/unit/checkout-default-address.test.tsx` |
| `327fcd81` | Product editor names the missing main-language name | `frontend/tests/unit/catalog-management.test.tsx` |
| `3351f2ce` | Checkout summary subtotal before discounts; promotion title as label | `frontend/tests/unit/checkout-summary-discount.test.tsx` |
| `24e0fdeb` | Coupon selection with an unknown cart token is 404 | suite `marketing_accounts` |
| `c0942e47` | Storefront language picker shows short autonyms | `frontend/tests/unit/storefront-language-label.test.ts` |
| `75932cb8` | German theme toggle label | copy only; localization gate |
| `6caff8ee` | Kit dialog wraps long tokens, actions stay visible | `frontend/tests/unit/kit-ui.test.tsx` |

## Open items (not fixed, with reason)

| Item | Why it was left |
| --- | --- |
| Variant option values are raw keys in cart, order and POS labels ("camel / L", "warm-gray / one-size") | Option values are not localizable content today; needs a content-model decision, not a UI patch |
| Mobile storefront header stacks into four rows (about a third of the first screen) | Needs a compact/menu header design |
| Cart lines show the discounted price while the summary lists the discount separately | Totals now add up; showing pre-discount line prices needs the undistributed line price from the server |
| Imprint page on a fresh shop shows only its heading (privacy and terms say "not published" and show a raw policy version hash) | Legal-page presentation owned by the parallel legal work; no wrong data is shown |
| Order numbers keep the legacy `RAC-` prefix | Changing the order number format before go-live needs an explicit decision |
| PDP kicker and footnote show the platform brand ("VEYNDO /") instead of the shop's | Branding decision |
| "0.0 · 0 reviews" with empty stars on products without reviews | Cosmetic |
| Coupon block shows "Gutscheincode" twice (summary and field label) | Cosmetic |
| Promotion editor: the sticky content-language box overlays fields while scrolling | Cosmetic; fields stay reachable |
| Content-language default differs between editors (product: interface language; flows/promotions: main language) | Needs a product decision on the default |
| Assistant run panel says an interrupted call "may already have been processed or charged" even when the model endpoint refused the connection | Wording owned by the durable-run contract |
| POS header offers "Open register mode" and an empty register picker before any register exists | Cosmetic; the empty state explains the next step |
| Bundled app named "Personalize this product" on its card and "Product personalization" in its detail | Cosmetic |
| Studio report range shown as ISO dates ("2026-10-28 — 2026-11-26") | Cosmetic |
| "Business" chip on My workspace at 2.7:1 contrast in dark mode | Cosmetic |
| Refund in Studio not walked in the browser; suite `browser_journeys` (which covers it) not run | Python Playwright is not installed and no downloads were allowed in this session |
| Assistant proposals with a real model not walked | No model service and no paid calls; covered by fixture suites such as `durable_agents` |
| `scripts/testing/operation_contracts.py` not run | Its Prettier import does not resolve through the Docker Node shim from the repository root |

## Screenshots

Storefront: `sf-home-de-desktop-light`, `sf-home-de-mobile-after-language-fix`,
`sf-product-variants-de`, `sf-product-de-mobile-before-language-fix`,
`sf-checkout-methods-de`, `sf-checkout-de-dates`, `sf-checkout-coupon-en`,
`sf-checkout-en-mobile`, `sf-checkout-default-address-de`, `sf-order-confirmed-de`,
`sf-account-addresses-de`, `sf-account-order-detail-de`, `sf-account-b2b-de`.

Studio: `studio-workspace-de-desktop-light`, `studio-order-detail-de`,
`studio-orders-en-desktop-dark`, `studio-orders-en-mobile-dark`,
`studio-product-create-before-fix-de`, `studio-app-consent-before-fix-en-dark`,
`studio-app-consent-after-fix-en-dark`, `studio-app-installed-en-dark`,
`studio-assistant-no-model-en-dark`, `studio-pos-empty-en-dark`.

All files are `docs/evidence/ux-2026-11-26/<name>.webp`.
