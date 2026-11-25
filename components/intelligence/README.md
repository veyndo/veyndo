# Intelligence component

Shop memory, knowledge retrieval, agent runs and planning, proposals, scenarios,
consent-bound behaviour signals and platform inference administration. Module
ownership is listed in [the source map](../../docs/source-map.md); the component
boundary and its declared foreign tables are in `components.json`.

## Open JSON

Fields are read through typed views (`veyndo_core::document::view`) and written as
typed `Serialize` documents, single literals or through `veyndo_core::document::merge`.
These values stay JSON by contract:

| Place | Reason |
| --- | --- |
| Model output (`service/planner.rs`, `service/read_rounds.rs`, `service/concierge.rs`, `service/scenarios/*`, `service/catalog_intake/*`) | Untrusted until `validate_native` admits it against the response schema; it is then read through typed views. |
| Tool, app and search results (`service/read_rounds.rs`, `domain/tool_trace.rs`, `knowledge/rerank.rs`) | Results of native reads, installed app tools and the retrieval index are passed through as evidence; the server adds only its own fields (`appView`, `rerankScore`). |
| Response schemas (`service/planner.rs`, `service/read_rounds.rs`, `service/agent_runs.rs`) | JSON Schema documents for the model contract and MCP, combined with the schemas the server contributes. |
| Platform inference configuration (`service/platform_ai.rs`) | The environment configuration comes from the cognition crate as JSON; stored overrides are laid over it. The saved document is typed (`StoredConfiguration`). |
| Agent run requests and checkpoints (`store/agent_runs.rs`) | Checkpoints are keyed by step (`model-*`, `operation-*`, `scenario-state`) and hold each step's own result. |
