# Frontend application entry

Only the application entry lives here; merchant, store and operator roots load independently. Shared design packages (tokens, UI components) live in [`kit/`](kit/README.md).

Files and their individual responsibilities are listed in [the generated source inventory](../../docs/module-inventory.md). Each file starts with its contract summary.

## Component UI folders

Studio and storefront screens live with their component, beside its Rust crate:
`components/<name>/ui/admin/<area>` and `components/<name>/ui/storefront/<area>`
(for example [`components/orders/ui/admin/orders`](../../components/orders/ui/admin/orders/README.md)).
This folder keeps the entry, the shells (`admin/shell`, `storefront/shell` and their
global styles), the operator console, shared building blocks and the design `kit/`.
`frontend/` builds, type-checks, tests and formats both roots as one tree;
`scripts/source-roots.mjs` gives the gates the same view, where
`components/orders/ui/admin/orders/X.tsx` reads as `admin/orders/X.tsx`. A component UI
imports `shared/` and `kit/` and the screens of other components only by relative
path; bare package imports resolve from this frontend (`resolve.dedupe` and
`tsconfig.json` `paths`). `admin/apps/AppConsent.tsx` and
`admin/channels/channel-i18n.ts` only re-export their owners for the hash-locked
Marketplace Studio app.

## Modules

- [`main.tsx`](main.tsx): Browser bootstrap only; application selection and error recovery live in application/.

## Verification

Run `npm run build`, `npm test`, `npm run test:coverage` and `npm run architecture` from `frontend/`. Coverage includes untested source files; a module in this folder is not automatically fully tested. See the root [testing guide](../../docs/testing.md) for backend integration and coverage limits.
