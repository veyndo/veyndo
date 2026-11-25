# UCP adapter

The UCP routes translate the native cart, checkout, consent and B2B owners into
UCP 2026-08-25 documents; they own no prices, permissions or purchasing rules.
`document.rs` defines the typed wire document (`UcpDocument` and its parts) and
projects a stored cart into it. `views.rs` holds the typed views of everything the
adapter reads: protocol requests, the priced native cart, stored order and consent
snapshots, shop links and checkout handoffs. Each view field reads exactly what
indexing the JSON did (an unexpected shape reads as null/`None`/empty); the unit
tests compare the typed output with the former indexed construction.
`profile.rs` publishes discovery and intersects platform profiles, `consent.rs`
validates and projects buyer consent, `negotiation.rs` routes B2B commands and
`cart.rs` issues cart transfers. Handlers serialize the typed document to JSON
once at the route boundary.

## Open JSON

These places keep `serde_json::Value` on purpose:

| Place | Reason |
| --- | --- |
| `UcpDocument.buyer`, `consent::buyer` | The buyer object is the client's open UCP object (contact, addresses, extensions), bounded to 16 KB and passed through; only the `consent` slot is typed (validated decisions on input, `ConsentChoice` on output). |
| Validated consent decisions in `consent::buyer` | Copied verbatim into the stored buyer after `validate` admitted exactly `granted`, `source` and `segments`. |
| `UcpDocument.currency`, line `id`/`item.id`/`title`/`quantity`, `UcpOrderLink.id` | Native values copied unchanged from the priced cart and order snapshot, including `null` for missing ones. |
| `UcpNegotiation.purchasing`, `quotes`, `quote_limit`, `result` | Native B2B purchasing, quote pages and command results passed through; their shapes belong to the B2B owner and its versioned schema `b2b-negotiation.json`. |
| `negotiation::command` request body | Forwarded unchanged to the native B2B command or checkout handoff, which validate it. |
| Platform profile capability lists (`PlatformUcp.capabilities`) | Remote, untrusted documents; each advertised entry is read through the lenient `PlatformCapability` view. |
| `schema()` | Serves the versioned `b2b-negotiation.json` JSON Schema file as-is. |
