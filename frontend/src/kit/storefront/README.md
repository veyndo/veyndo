# kit/storefront

The storefront kit. Each part has one typed contract (`definePart`): a Zod schema
of serializable props and a configuration schema (what a merchant sets in the
Visual Builder, such as which product), from which come the TypeScript types, validation of untrusted
props (attributes, stored blocks, model output), the JSON Schema used by builder
forms, MCP tools and the server, and the React component. Parts are styled through
design tokens only and carry their own four-language vocabulary; hosts pass the
locale and data, and receive buyer intents as callbacks (`onAddToCart`,
`onCheckout`).

## Modules

- [`index.ts`](index.ts): The public interface and the `parts` list.
- [`contract.ts`](contract.ts): `definePart` and shared field shapes (locale, currency, amount, safe URL).
- [`storefront-i18n.ts`](storefront-i18n.ts): Part vocabulary in English, German, French and Spanish; money formatting.
- [`ProductCard.tsx`](ProductCard.tsx): Product card.
- [`ProductSummary.tsx`](ProductSummary.tsx): Purchase area of a product page.
- [`CartSummary.tsx`](CartSummary.tsx): Priced cart lines with quantity controls, discounts, taxes, totals and delivery window from the server cart.
- [`CheckoutSteps.tsx`](CheckoutSteps.tsx): Checkout progress; never implies a confirmed payment.
- [`AccountOverview.tsx`](AccountOverview.tsx): The account home (greeting, counts, recent orders) and the order list part.
- [`elements.tsx`](elements.tsx): `registerPartElements` defines `<veyndo-<id>>` web elements that validate their `props` attribute, upgrade server-rendered fallback markup (islands) and emit `veyndo:add-to-cart`, `veyndo:checkout`, `veyndo:quantity` and `veyndo:open` events.
- [`PartForm.tsx`](PartForm.tsx): The Visual Builder form of a part's configuration, generated from its configuration JSON Schema; hosts supply localized labels and pickers for special formats such as `product-id`.
- [`standalone-styles.ts`](standalone-styles.ts): Injects the token, ui and part styles for entries that load without the app stylesheet.
- [`mcp-app.tsx`](mcp-app.tsx): The MCP App view: speaks the MCP Apps bridge (`2026-01-26`), renders the part payload of a tool result and sends buying to the shop through `ui/open-link`.
- [`mcp-entry.ts`](mcp-entry.ts): Build entry of the view bundle (`vite.mcp.config.ts` → `dist/mcp/storefront-parts.js`).
- [`embed.ts`](embed.ts): The standalone entry built as `/assets/elements.js`: it injects the token, ui and part styles once and defines every part element.
- [`storefront.css`](storefront.css): Part styles; the architecture check refuses colour literals here.

Prices, stock and totals always come from the server; parts only show them.

## Where the parts run

- **Storefront app.** Every purchase surface renders kit parts through thin
  adapters: product tiles (`shell/ProductTile`), the product page purchase area
  (`catalog/ProductPurchase`, the buy box with variants, quantity prices and app
  slots), the checkout order review and order completion (cart summary), checkout
  steps, the account home and order lists, and the account order detail. Adapters
  translate shop data and keep navigation, app slots and cart rules; the storefront
  theme styles the kit classes. The chat purchase review (`ConversationPurchase`)
  stays its own explicit confirm flow, and B2B quote lines belong to the B2B owner.
- **Site builder.** Pages place the product card and the buy box as `kind: "part"`
  blocks (see `extensions/apps/site-builder/README.md`); the inspector is
  `PartForm`, and Rust validates the stored configuration against the schema
  generated from these contracts.
- **Islands.** For deep-linked product pages Rust (`src/storefront_islands.rs`)
  renders `<veyndo-product-summary>` into the document root: props from the Store
  API product owner and escaped, language-neutral fallback markup, so crawlers,
  agents and visitors without JavaScript see the product. The storefront app then
  mounts over it; a host that loads `elements.js` hydrates it instead.
- **Embedding.** Another site loads
  `<script type="module" src="https://<shop>/assets/elements.js">` and writes
  `<veyndo-product-card props='{"product":{…}}' locale="de-DE">`. The element
  validates the attribute with the part schema (invalid props keep the fallback
  and set `data-invalid`) and emits `veyndo:add-to-cart`, `veyndo:checkout`,
  `veyndo:quantity` and `veyndo:open` events; the host decides what they do. Cross-origin hosts need the shop to serve
  the script with CORS.

- **Agents (MCP Apps).** The hosted `/mcp` server links `catalog.detail` and
  `catalog.search` to the view `ui://veyndo/kit/storefront`
  (`_meta.ui.resourceUri`, plus the `openai/outputTemplate` alias ChatGPT still
  honours) and serves it as a self-contained `text/html;profile=mcp-app` document
  built from these contracts (`src/mcp/storefront_views.rs`). Each result carries
  its part props in `_meta["ai.veyndo/part"]`, derived from the same Store API
  answer the model sees; hosts do not pass `_meta` to the model. The view never
  takes payment: add to cart and checkout open the shop's own product page or
  checkout, and only HTTPS links leave the sandbox. Images and links need
  `COMMERCE_PUBLIC_ORIGIN`; without it the view shows the parts without them.

## Verification

Run `npm test -- kit-storefront` and `npm run architecture` from `frontend/`.
