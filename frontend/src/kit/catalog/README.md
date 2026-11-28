# kit/catalog

The versioned component catalog (`veyndo:components/1`) and the one renderer of
UI documents. Every reusable kit component and the storefront patterns agents may
use are declared once in [`entries.tsx`](entries.tsx) and
[`patterns.tsx`](patterns.tsx): a Zod schema of the document props, slots with
the components they accept, events with payload schemas, the accessibility
contract, the surfaces that admit the entry, examples and the render function.
From that one source come the [catalog document](catalog.json) agents and other
languages read, the TypeScript prop types in
[`../ui/props.generated.ts`](../ui/props.generated.ts) that kit/ui itself uses,
and the handbook's [component gallery](../../../../docs/component-gallery.md).
The `component-catalog` unit test fails when any of the three drifts; run
`WRITE_COMPONENT_CATALOG=1 npm test -- component-catalog` after a change.

Agents, apps and MCP Apps send documents in three formats: `veyndo-ui/1`
(nested JSON), A2UI v1.0 messages or OpenUI Lang. `CatalogView` reads all three
into one tree, validates every node against the catalog and the host's
`Admission` (surface, data, actions, origins, host components) and renders only
what passed. Each refusal shows a localized notice in its place. See
[the design system](../../../../docs/design-system.md) and
[agentic component UI](../../../../docs/agentic-component-ui.md#one-catalog-one-renderer).

## Modules

- [`index.ts`](index.ts): The public interface; import only this file from outside `kit/`.
- [`define.ts`](define.ts): The entry shape (`defineEntry`), surfaces, slot and event rules, OpenUI argument order.
- [`fields.ts`](fields.ts): Safe field shapes: plain text without markup, `safeUrl`, identifiers, tones, spacing, actions.
- [`entries.tsx`](entries.tsx): Entries of the kit/ui components and the document-only content entries (Text, Heading, Image, TabPanel).
- [`patterns.tsx`](patterns.tsx): The storefront parts as patterns; their commerce props must be bound to host data.
- [`catalog.ts`](catalog.ts): The catalog id, version, limits, the catalog document and the generated TypeScript source.
- [`catalog.json`](catalog.json): Generated catalog document; do not edit.
- [`document.ts`](document.ts): Reads `veyndo-ui/1`, A2UI v1.0 and OpenUI Lang into one untrusted tree within the limits.
- [`validate.ts`](validate.ts): Validation against the catalog and the admission; JSON Pointer bindings to admitted data.
- [`CatalogView.tsx`](CatalogView.tsx): The renderer with per-node error boundaries and localized fallbacks.
- [`catalog-i18n.ts`](catalog-i18n.ts): Fallback, gallery and sample text in English, German, French and Spanish.
- [`Gallery.tsx`](Gallery.tsx): The living gallery shown in Studio (Developer → Components).
- [`element.tsx`](element.tsx): `<veyndo-document>`, the storefront surface's web element; actions arrive as `veyndo:action` events.
- [`embed.ts`](embed.ts): The `/assets/elements.js` entry: part elements and `<veyndo-document>` with their styles.
- [`mcp-view.tsx`](mcp-view.tsx): The MCP App view's document support (`_meta["ai.veyndo/ui"]`) on the mcp-app surface.
- [`mcp-entry.ts`](mcp-entry.ts): The MCP App view bundle entry (`dist/mcp/storefront-parts.js`).
- [`standalone-styles.ts`](standalone-styles.ts): Injects token, ui, part and document styles for standalone entries.
- [`catalog.css`](catalog.css): Document and gallery styles; design tokens only.

## Surfaces

| Surface      | Entry point                                                   | Admission                                                                              |
| ------------ | ------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `studio`     | `GeneratedWorkspace` (A2UI/OpenUI working views), the gallery | Native controls as host components; the workspace catalog id                           |
| `storefront` | `<veyndo-document>` in `/assets/elements.js`                  | Data, actions and origins the embedding page sets as attributes                        |
| `app`        | `ConversationAppViews` when an app view declares `document`   | Only the arguments and result of the read the host ran                                 |
| `mcp-app`    | The MCP App view (`ui://veyndo/kit/storefront`)               | The `ai.veyndo/ui` payload's data and shop origins; links leave through `ui/open-link` |

## Verification

Run `npm test -- component-catalog component-renderer component-surfaces kit-ui`
and `npm run architecture` from `frontend/`. The nightly
`python3 scripts/component_gallery.py` renders the gallery in Chromium for
contrast checks and visual snapshots.
