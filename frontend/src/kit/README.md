# kit

Modular frontend packages shared by Studio, the storefront and the operator console.
Each package exposes its public interface through `index.ts`; code outside `kit/`
imports only that file. Packages import only lower packages:
`tokens` ← `ui` ← `storefront` ← `catalog`; `api` and `query` stand alone. A package never imports
`shared/`, `admin/`, `storefront/`, `platform/` or `application/`.

| Package                              | Responsibility                                                                              |
| ------------------------------------ | ------------------------------------------------------------------------------------------- |
| [`tokens`](tokens/README.md)         | Design tokens and their generated stylesheet                                                |
| [`ui`](ui/README.md)                 | Accessible atoms, molecules and headless primitives; label-free, text comes from the caller |
| [`storefront`](storefront/README.md) | Storefront parts with typed contracts, web elements and the MCP App bridge                  |
| [`catalog`](catalog/README.md)       | The versioned component catalog, document validation, the one renderer and the gallery      |

## Verification

Run `npm run architecture` (ownership, layering, token use) and `npm test` from `frontend/`.
