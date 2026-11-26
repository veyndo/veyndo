# Personal Studio presentation

`components/studio/src/domain/workspace.rs` validates bounded work areas, card IDs, safe grid projections and
personal goals. `../personal.rs` is the shared HTTP/MCP entry point; `components/studio/src/store/workspaces.rs` owns the revisioned store.

Preferences use the current personal session and active membership, forced
tenant-and-actor RLS, optimistic revisions and twenty retained versions. An
integration credential is not a personal session. Preferences are presentation
state, not roles, jobs, customer data, sandbox units or a second saved-grid store.
The native task and Flow owners execute reviewed work; goals themselves do not
schedule or cancel external operations.
