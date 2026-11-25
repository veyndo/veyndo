# admin/intelligence

Evidence, graph and learning read models; curated relationships and observed behavior remain distinguishable.

Files and their individual responsibilities are listed in [the generated source inventory](../../../../docs/module-inventory.md). Each file starts with its contract summary.

## Modules

- [`ExternalKnowledge.tsx`](ExternalKnowledge.tsx): Private connected-app evidence browser shows active-source provenance without exposing it to shoppers.
- [`KnowledgeExplorer.tsx`](KnowledgeExplorer.tsx): Product-centred evidence inspector reads canonical facts and graph relationships beyond overview sampling.
- [`KnowledgeFacts.tsx`](KnowledgeFacts.tsx): Canonical catalogue facts shown alongside graph evidence; prices and inventory come from current product state.
- [`KnowledgeOverview.tsx`](KnowledgeOverview.tsx): Whole-shop knowledge census, operational next steps and provenance activity; examples never masquerade as learned facts.
- [`KnowledgePreview.tsx`](KnowledgePreview.tsx): No-model evidence preview makes customer/private retrieval boundaries and missing facts inspectable.
- [`KnowledgeSources.tsx`](KnowledgeSources.tsx): Searchable cursor source library, guarded lifecycle decisions and single-language source editing.
- [`KnowledgeView.tsx`](KnowledgeView.tsx): Unified knowledge workspace connects sources, product evidence, observations and no-model retrieval previews.
- [`MemoryView.tsx`](MemoryView.tsx): Durable co-purchase evidence and revision-bound merchant decisions, with simulation labels and explicit confirmation.
- [`SourceEditor.tsx`](SourceEditor.tsx): Single-language source editor with inherited fields, product lookup and private-first API/file ingestion.
- [`knowledge-types.ts`](knowledge-types.ts): Typed knowledge read models preserve source ownership, revisions, sampling and privacy boundaries.

## Studio registration

[`studio-extension.ts`](studio-extension.ts) declares this area's Studio routes and slot contributions for [the registry](../shell/registry/README.md); [`KnowledgeRoute.tsx`](KnowledgeRoute.tsx) adapt the Studio context to the area's views.

## Styles

Handwritten area styles moved from `admin/styles` (unchanged rules; rebuilding on `kit/ui` follows).

- [`knowledge-evidence.css`](knowledge-evidence.css): Product evidence, retrieval excerpts, observed pairs and responsive knowledge layouts.
- [`guardrails.css`](guardrails.css): Native pricing policy workbench: merchant units, bounded product lookup and shared Studio tokens.
- [`behavior.css`](behavior.css): Consent-bound insights use the existing Studio tokens and a responsive, inspectable visual hierarchy.
- [`knowledge.css`](knowledge.css): Unified knowledge workspace: evidence-first hierarchy, accessible cards and theme-aware responsive layouts.
- [`knowledge-sources.css`](knowledge-sources.css): Knowledge source library/editor layouts: single-language forms and lifecycle controls.
- [`autopilot.css`](autopilot.css): Autopilot summary: level per area, daily meters and today's autonomous price changes.
- [`sovereign.css`](sovereign.css): Sovereign AI report: mode state and one row per model route.

## Verification

Run `npm run build`, `npm test`, `npm run test:coverage` and `npm run architecture` from `frontend/`. Coverage includes untested source files; a module in this folder is not automatically fully tested. See the root [testing guide](../../../../docs/testing.md) for backend integration and coverage limits.

The connected cognition editors use native APIs: `EvidenceReview.tsx` reviews
current document-bound claims; `GuardrailSettings.tsx` edits canonical commerce
settings and currency scales; `ExperimentStudio.tsx` registers and controls native
layout experiments with clear preliminary/final readouts. Each uses the shared
typed four-language vocabulary. Revision and permission failures remain visible.
See [cognitive commerce](../../../../docs/cognitive-commerce.md) for tested scope.

The native price-boundary workspace uses `GuardrailSettings`, bounded catalogue
lookup and currency-aware cards. `guardrail-model.ts` only checks editor structure;
Rust owns price/autonomy admission. Grouped `KnowledgeNavigation` has a compact
selector at narrow widths. See [the user workflow](../../../../docs/knowledge-workspace.md#control-ai-price-changes).
