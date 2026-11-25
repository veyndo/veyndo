# identity UI

The user interface of the `identity` component, beside its Rust crate. It is part of
the one frontend source tree that `frontend/` builds, type-checks, tests and gates
(see [the frontend source guide](../../../frontend/src/README.md#component-ui-folders)).

- Studio areas: [`admin/team`](admin/team/README.md).

Each area keeps its own folder contract (`README.md`). Imports use relative paths;
shared building blocks come from `frontend/src/shared` and `frontend/src/kit`.
