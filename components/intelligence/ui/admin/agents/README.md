# admin/agents

Agentic distribution capabilities and account/endpoint connection visibility.

Files and their individual responsibilities are listed in [the generated source inventory](../../../../../docs/module-inventory.md). Each file starts with its contract summary.

## Modules

- [`AgentsView.tsx`](AgentsView.tsx): AgentsView renders verified shop state and typed user actions.

## Studio registration

[`studio-extension.ts`](studio-extension.ts) declares this area's Studio routes and slot contributions for [the registry](../../../../../frontend/src/admin/shell/registry/README.md); [`AgentsRoute.tsx`](AgentsRoute.tsx) adapt the Studio context to the area's views.

## Verification

Run `npm run build`, `npm test`, `npm run test:coverage` and `npm run architecture` from `frontend/`. Coverage includes untested source files; a module in this folder is not automatically fully tested. See the root [testing guide](../../../../../docs/testing.md) for backend integration and coverage limits.
