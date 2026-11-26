# admin/styles

Studio visual system and merchant operational layouts. Entry imports preserve cascade order.

Files and their individual responsibilities are listed in [the generated source inventory](../../../../docs/module-inventory.md). Each file starts with its contract summary.

## Modules

- [`forms.css`](forms.css): Studio-owned form primitives load at the composition root, independent of lazy workspace history.
- [`markdown-editor.css`](../../shared/styles/markdown-editor.css): Shared visual/Markdown product editor treatment using existing Studio theme tokens.
- [`operations.css`](operations.css): Operational screens share the studio's light surface and clear focus states.
- [`studio-sign-in.css`](studio-sign-in.css): Dedicated access surface: responsive Veyndo login and native modal reauthentication.
- [`studio.css`](studio.css): Ordered studio stylesheet entry; domain rules live in the adjacent folder.
- [`workspace-polish.css`](workspace-polish.css): Consistent Studio density, readable hierarchy and independently scrollable navigation across workspaces.

## Verification

Run `npm run build`, `npm test`, `npm run test:coverage` and `npm run architecture` from `frontend/`. Coverage includes untested source files; a module in this folder is not automatically fully tested. See the root [testing guide](../../../../docs/testing.md) for backend integration and coverage limits.

Catalogue base and responsive styles load with both native editors, including direct
Canvas handoffs. `canvas-commerce-editor.css` owns the shared wrapper spacing;
opening App Studio or the product directory first is never a styling prerequisite.

The generic Studio button reset uses `:where(.studio)` so feature controls keep
border/background states regardless of lazy stylesheet order. Explicit primary
and secondary action classes keep their existing priority. `team-workspace.css`
owns responsive account, shop creation and access-panel layout; it does not alter
roles, sessions or key authority.
