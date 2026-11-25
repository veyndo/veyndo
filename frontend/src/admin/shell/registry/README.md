# admin/shell/registry

The Studio extension registry. Every Studio area declares what it adds to the shell in
its own `studio-extension.ts`, the same way an installed app declares surfaces in its
manifest: routes (with rights, icon, sidebar group and position, and what the route
needs before it can render), and contributions to slot locations (page slots, dashboard
cards and form fields). `first-party.ts` registers the descriptors in route order; the
shell renders only what the registry returns.

Slot locations share their names with app surfaces (`admin.order`,
`admin.order.general`, `admin.product.general`), so `StudioSlot` renders first-party
contributions and installed apps at the same place. `admin.dashboard` is Studio's own.

Route views are code-split: a descriptor registers its view with `lazy(import(...))`.
The architecture check does not count these registration loaders as initialisation
edges, so a route view may use shell helpers that read the registry; static cycles
still fail.

## Modules

- [`types.ts`](types.ts): Route, contribution, rights and slot context types.
- [`define.ts`](define.ts): `defineStudioExtension`, `contribute` and the `allows` rights rule.
- [`registry.ts`](registry.ts): `createStudioRegistry` validates descriptors and answers route, navigation and slot queries.
- [`first-party.ts`](first-party.ts): The first-party descriptors in route order.
- [`StudioSlot.tsx`](StudioSlot.tsx): Renders one slot location for the current rights, then the apps at that location.

Presentation admission only: the server checks shop, identity and current rights for
every request.

## Verification

Run `npm test -- studio-registry` and `npm run architecture` from `frontend/`.
