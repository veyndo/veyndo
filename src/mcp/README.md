# MCP transport

`transport.rs` preserves app MCP opt-in and request-scoped read memoization.
It calls the existing capability dispatch; it does not own prices or permissions.
The parent `mcp.rs` owns request authentication and JSON-RPC framing.

`protocol.rs` reads the JSON-RPC envelope and `tools/call` parameters through
typed views (`Envelope`, `CallParams`) and builds tool results from `ToolResult`;
`mcp.rs` announces `ServerCapabilities`.

## Open JSON

| Place | Reason |
| --- | --- |
| JSON-RPC `id` | Echoed unchanged; JSON-RPC 2.0 allows a string, number or null. |
| `RpcRequest.params` (`Raw`) and `tools/call` `arguments` | Tool inputs are open by protocol; each native operation or app tool validates them against its own input schema. |
| Results (`structuredContent`, `result`, resource contents) | The native operation's or app's output passed through unchanged. |
| App tool descriptors in `tools/list` | Published by the app gateway owner; only `name` is read (`ToolName`). |
| `initialize` client `capabilities.extensions` | Open MCP extension map; only the MCP Apps `mimeTypes` list is read. |
| `storefront_views.rs` resource documents | MCP Apps resource payloads built for the client UI, not read back. |
