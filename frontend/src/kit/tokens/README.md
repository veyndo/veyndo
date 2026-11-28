# kit/tokens

Design tokens for every Veyndo surface: colours (light and dark), spacing, radii,
type, shadows and motion, plus brand themes that replace the light tokens for one
shop. `tokens.json` is the only source of token values; `scripts/tokens.mjs`
generates `tokens.css` and `npm run architecture` fails when the two differ.

## Modules

- [`tokens.json`](tokens.json): The token values. Light colours equal the former Studio palette. `motion-intensity` holds the off, subtle and expressive values of the motion tokens.
- [`tokens.css`](tokens.css): Generated custom properties (`--vy-*`) on `:root`, `[data-theme="dark"]` and `[data-theme="system"]` under a dark colour scheme; `[data-motion="off|subtle|expressive"]` selects a motion intensity; reduced motion sets every duration, distance and press scale to rest, whatever the intensity.
- [`motion.css`](motion.css): Motion primitives on the tokens: `.vy-enter` (entrance, only inside `[data-motion]`), `[data-vy-reveal]` (scroll reveal), `.vy-lift` (hover) and `.vy-press` (press).
- [`motion.ts`](motion.ts): Motion at run time: `observeReveal` marks reveal elements pending only when motion is allowed, `playPageEnter` animates the main region after a route change, `motionIntensity` reads stored values defensively.
- [`brand-theme.ts`](brand-theme.ts): Brand themes: WCAG contrast and OKLCH colour maths, deterministic palette extraction from pixels, the logo-shape radius suggestion, vetted font stacks and pairings, the radius/density scales, `deriveBrandTheme`, the typed document schema, `parseBrandTheme` for untrusted input, `brandVariables` and `safeThemeVariables`.
- [`index.ts`](index.ts): Typed access to the tokens and their custom property names; re-exports the brand and motion modules.

## Rules

- Components in `kit/ui` and `kit/storefront` use only `var(--vy-*)` for colours; the architecture check refuses colour literals there.
- Change a value in `tokens.json`, run `node scripts/tokens.mjs`, commit both files.
- A brand theme's contrast pairs (`contrastPairs`) and pairings (`fontPairings`) are mirrored by the Rust validator in `extensions/apps/site-builder/brand_theme.rs`; change both together.
- Font stacks name installed families only; no web font is downloaded.

## Verification

Run `npm run architecture` and `npm test` from `frontend/` (`brand-theme`, `motion`, `brand-surfaces`, `site-brand`).
