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
- [`CartSummary.tsx`](CartSummary.tsx): Cart lines and totals from the priced server cart.
- [`CheckoutSteps.tsx`](CheckoutSteps.tsx): Checkout progress; never implies a confirmed payment.
- [`AccountOverview.tsx`](AccountOverview.tsx): Buyer name and recent orders.
- [`elements.tsx`](elements.tsx): `registerPartElements` defines `<veyndo-<id>>` web elements that validate their `props` attribute, upgrade server-rendered fallback markup (islands) and emit `veyndo:add-to-cart` / `veyndo:checkout` events.
- [`PartForm.tsx`](PartForm.tsx): The Visual Builder form of a part's configuration, generated from its configuration JSON Schema; hosts supply localized labels and pickers for special formats such as `product-id`.
- [`storefront.css`](storefront.css): Part styles; the architecture check refuses colour literals here.

Prices, stock and totals always come from the server; parts only show them.

## Verification

Run `npm test -- kit-storefront` and `npm run architecture` from `frontend/`.
