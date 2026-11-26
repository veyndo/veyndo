# Inference component

`veyndo-cognition` owns model provider adapters, inference budgets, prompt
provenance and image generation. It has no commerce write capability; see
[the source map](../../docs/source-map.md).

## Owner tables

The inference crate has no tables of its own and no SQL. The live shop and its
agent stop switch come from `veyndo_core::store::live_shop`, the allocation audit
event from `veyndo_core::store::outbox`. Identity depends on this crate, so agent
key budgets and the operator configuration (`platform_ai`, owned by shop) are
reached through the `InferenceHost` port (`src/ports.rs`), implemented in the
server (`src/ports/inference.rs`, backed by `veyndo_identity::store::agent_budgets`
and `veyndo_shop::store::platform_ai`) and registered at startup. A process
without the registered host has no durable model admission, exactly as one
without a database.

## Open JSON

Provider responses and host documents are read through typed serde views; the
members the host sets on provider requests are typed structs merged with
`veyndo_core::document::merge`. These stay open JSON by contract:

| Place | What stays open | Reason |
| --- | --- | --- |
| `src/inference/schema.rs` (`strict_schema`, `restore`) | Caller JSON Schemas and model answers | Every keyword and property name comes from the caller; the adaptation walks arbitrary schemas and restores arbitrary values. |
| `src/inference/request.rs`, `protocol.rs`, `tools/request.rs` | Provider request bodies | Each provider defines its own wire format; the host sets `tools`, `input`, `include`, `parallel_tool_calls`, `system` and `prompt_cache_key` and removes structured-output members by name. |
| `src/inference/request.rs` (`usage`) | Provider usage counts | Passed on as reported; the host adds `prompt` and `toolContinuation`. |
| `src/inference/settings.rs` | The stored platform AI document | Operator configuration read through `PlatformAi`; opening keys rewrites only `providers.<id>.key`. |
| `src/inference/tools/*` | Tool continuation history and results | Opaque provider reasoning and tool results correlated by id and replayed unchanged. |
