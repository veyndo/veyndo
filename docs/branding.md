# VEYNDO identity and compatibility

**Product:** VEYNDO. **Merchant workspace:** VEYNDO Studio.
**Operator console:** VEYNDO Platform.
[Canonical repository](https://github.com/veyndo/veyndo) ·
[Documentation](https://www.veyndo.ai/docs/).

The VEYNDO identity uses the approved Living V symbol and custom outlined
geometric lettering. Two offset solid elements form one directed V, expressing
connected intelligence turning understanding into action. The palette is
graphite `#171B18`, warm white `#F5F3EC` and Shopware blue `#189EFF`. The V mark is used
for favicons and compact surfaces. UI and site favicons use identical copies checked by
`scripts/branding.py`; wordmarks have light and dark variants.
[Mark](brand/veyndo-mark.svg), [wordmark](brand/veyndo-logo.svg),
[dark wordmark](brand/veyndo-logo-dark.svg). `shared/ui/Brand.tsx` owns the UI
identity and places the Studio label below the wordmark within the available
width. Tenant-owned company logos and merchant shop names are independent.

The [repository header](brand/veyndo-readme.svg) uses the same vector identity
and the approved subtitle “Commerce that thinks and acts.” README product
images are unedited captures of the actual prototype.

## Package and deployment naming

The Rust crate and HTTP binary are `veyndo`; Rust imports use `veyndo::`.
The frontend package is `veyndo-experience`. The application image uses `veyndo`; fresh local database/search containers use
standard PostgreSQL and Qdrant images. Fresh local installations use Compose project `veyndo` and
container `veyndo-postgres-1`. Start and worker commands,
MCP examples, deployment files and CI use these names consistently.

`scripts/dev.sh` detects an older `rust-ai-commerce` Compose project and reuses it;
an explicit `COMPOSE_PROJECT_NAME` takes precedence. A database created by an earlier
release is refused at setup, so start a new project for this release. Integration verification propagates its
selected container to every child suite, so both new and existing instances can
be checked. To stop an older installation use its actual Compose project name;
never delete its database volume to perform a rename.

## Deliberately stable contracts and historical records

- Existing `atelier` / `workshop` tenant IDs are data identities. User-created
  shop names, company brands, documents and orders are preserved.
- Browser `rac-*` session/cart/customer keys and internal `x-rac-*` principal
  headers retain their existing security behavior; renaming them is unnecessary
  and would invalidate sessions or integrations. Client principal headers remain
  untrusted and cannot grant access.
- The renamed trigger contracts and diagnostic function are part of the component
  baselines; a release replaces earlier installations, so no ledger compatibility
  for the old names is kept.
  No orders, customer records, IDs or audit history are rewritten.
- Existing Qdrant projections are preserved through additive collection aliases.
  Run the migration-only job with `QDRANT_NAMESPACE_MIGRATE_FROM` set to the
  deployment's actual previous namespace, alongside the existing `QDRANT_URL`
  and optional `QDRANT_API_KEY`. Leave it empty on fresh installs and serving
  replicas. The job validates the exact v2 collection names, model hash suffix,
  vector dimension and cosine geometry before changing any aliases. Existing
  canonical collections or conflicting aliases stop the job. It does not
  delete collections, change point IDs or spend credits on re-embedding.
  Repeating the job is idempotent; the regular deletion worker includes aliases
  so customer/merchant deletion still removes the appropriate tenant's points.
  The existing real PostgreSQL/Qdrant managed-search suite checks this migration
  through the compiled setup job and the normal search/deletion paths.
- The current WIT package is `veyndo:commerce@1.0.0`. Existing immutable
  component packages can retain their approved source and digests: configure
  their actual previous namespace in `COMPONENT_LEGACY_NAMESPACES` during
  cutover. Only explicitly listed v1 namespaces are admitted. They reach the
  same bounded cart/product/app-record host through structurally checked WIT
  types; memory, tables, fuel, read-call quotas, money and output validation
  remain unchanged. Unknown namespaces/versions are rejected. No installed
  package is rewritten, resigned or granted additional permissions.
- Git history retains the original dated evidence. Current documentation uses
  the canonical product name and links. Regenerated media carries its actual
  new capture source, date and hashes; no old capture is presented as new.

`python3 scripts/branding.py` checks package/runtime paths, current public
identity, shared SVG assets, source ownership and the absence of stale platform
branding in the current frontend, website and README. It does not assert a
trademark clearance or production readiness.

The central UI palette lives in `frontend/src/shared/ui/brand.css` and is loaded by
the browser bootstrap. Use `--veyndo-blue` for the identity/focus and
`--veyndo-blue-text` (#0B539B) for readable light-surface actions. Dark surfaces
use #189EFF with #06325F button text. Studio density styles must inherit the palette
rather than declaring another theme. SVG copies remain byte-identical.

The selected primary is documented in [Shopware's official brand guide](https://assets.shopware.com/media/website/press_materials/brochure/SW_CD_Design_Manual.pdf).

## Shop brand themes

A merchant can give the standard storefront a complete theme from their own brand on a
button press. The Veyndo identity above belongs to Studio and the product; a shop brand
theme changes only that shop's storefront.

**Where it lives.** The theme is a typed token document, `theme.brand`, on the shop's
home page record of the Shop design app (global `*` home or a channel home; channel
homes override the global one). It is therefore saved, versioned, restored and
published exactly like other design drafts: private sandbox draft, record history,
staging diff and the reviewed selective release. No database table or migration was
added; the page `theme` was already a JSON field. Brand documents are refused on any
other page.

**How it is derived.** Everything that decides the theme is deterministic code in
`frontend/src/kit/tokens/brand-theme.ts`:

1. The browser scales each chosen image (company logo, uploads, product images) to at
   most 64×64 pixels. Uploads are read locally and not stored.
2. `extractPalette` sorts opaque pixels into 4-bit colour buckets, groups chromatic
   colours into 15° hue bins weighted by chroma (the logo counts twice) and takes the
   strongest bin as primary and a bin at least 45° away as secondary. A logo without
   chromatic colour is a monochrome brand. The same pixels always give the same palette.
3. `deriveBrandTheme` builds the light scheme in OKLCH around the primary hue: tinted
   backgrounds, text, muted text, accent, accent text, soft and strong accents, border
   and focus. Foregrounds are darkened step by step until every pair in
   `contrastPairs` meets WCAG 2.x AA: 4.5:1 for text, links and button labels, 3:1 for
   the focus ring. A very light brand colour (yellow, pale cyan) therefore becomes a
   darker accent of the same hue; the original stays in `palette`.
4. Typography is a vetted heading/body pairing (`fontPairings`: classic, editorial,
   modern, friendly, crafted, timeless, technical) over font stacks of installed
   families only, so no web font is fetched and no shopper data leaves the shop. The
   colours suggest a default pairing and motion intensity; the logo silhouette
   suggests sharp, soft or round corners. Spacing is compact, regular or airy.

**Validation.** Model output and every other writer are untrusted. `parseBrandTheme`
(browser) and `extensions/apps/site-builder/brand_theme.rs` (server, on every save
through Studio, HTTP, MCP or a design-agent proposal) accept only the exact document
shape, lowercase `#rrggbb` colours, a vetted pairing whose fonts match it, known
scales, at most six source references and all contrast pairs. Brand themes are
created without a model call; a design-agent proposal that touches the theme passes
the same validator, so a model can at most choose among vetted options.

**What follows the theme.** `brandVariables` sets the light `--vy-color-*`, font,
radius and spacing tokens and the `--veyndo-blue-*` aliases on the shop root, so the
native header, hero, product tiles, product page, cart, checkout, account, legal pages,
storefront kit parts and native app surfaces in the storefront all read it. The legacy
page fields (`accent`, `background`, `foreground`, `font`, `radius`, `spacing`) are set
from the same document so site sections match. MCP App views in the shopper
conversation receive the shop's `--vy-*` overrides as host context `styles.variables`;
the kit view applies only plain `--vy-*` values (`safeThemeVariables`).
Order emails are plain text and the receipt PDF has no colour hook; they keep using the
company logo and name and do not read the theme. No new renderer was built for them.

## Motion language

Motion is a small set of tokens in `kit/tokens/tokens.json` (`motion` and
`motion-intensity`) with four primitives: entrance (`.vy-enter`), scroll reveal
(`[data-vy-reveal]`), hover lift and press, and the page transition (`playPageEnter`
after a storefront route change). A shop selects `off`, `subtle` or `expressive` as
`theme.motion` on its home page; the storefront root then carries `data-motion`.

- Without a selection a shop keeps its earlier behaviour: buttons lift and press as
  before (the subtle values), and no entrance, reveal, card lift or page transition runs.
- `off` stills everything, including buttons.
- `prefers-reduced-motion: reduce` sets every duration and distance to zero for all
  intensities, and the reveal script then marks content shown immediately.
- Content is never hidden without JavaScript: reveal elements become `pending` only
  when the script runs and motion is allowed.
