# kit/tokens

Design tokens for every Veyndo surface: colours (light and dark), spacing, radii,
type, shadows and motion. `tokens.json` is the only source; `scripts/tokens.mjs`
generates `tokens.css` and `npm run architecture` fails when the two differ.

## Modules

- [`tokens.json`](tokens.json): The token values. Light colours equal the former Studio palette.
- [`tokens.css`](tokens.css): Generated custom properties (`--vy-*`) on `:root`, `[data-theme="dark"]` and `[data-theme="system"]` under a dark colour scheme; reduced motion sets durations to zero.
- [`index.ts`](index.ts): Typed access to the tokens and their custom property names.

## Rules

- Components in `kit/ui` and `kit/storefront` use only `var(--vy-*)` for colours; the architecture check refuses colour literals there.
- Change a value in `tokens.json`, run `node scripts/tokens.mjs`, commit both files.

## Verification

Run `npm run architecture` and `npm test` from `frontend/`.
