# studio

This folder owns the app-specific studio implementation. See the parent app README for its public contract, isolation and release boundary. It uses existing host requests, translations and commerce operations.

`ImageProperties.tsx` edits the normal page block image, accessibility alt text and
localized caption. The shared proposal editor does not save or publish the page.

`BrandThemeDialog.tsx` creates a brand theme on a button press: it offers the company
logo, local uploads (`brand-images.ts` decodes them in the browser; nothing is stored)
and product images, derives the theme with the deterministic `kit/tokens` brand
functions, shows the current and the proposed storefront side by side and applies the
result only to the open home-page draft. `AppearanceControls.tsx` opens it on home pages
and selects the shop's motion intensity. Vocabulary: `brand-i18n.ts`.
