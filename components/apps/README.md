# App platform component

`veyndo-app-platform` owns app packages, managed app data, UI surfaces, Wasm
extensions and the app builder. Module ownership is listed in
[the source map](../../docs/source-map.md); the shared app/action contract is in
[the app platform guide](../../docs/app-platform.md).

## Owner tables

The app platform writes only its own tables. Reads and writes of other owners go
through their store functions (`veyndo_catalog::store::app_references`,
`imported_lines`, `veyndo_orders::store::app_orders`,
`veyndo_checkout::store::app_carts`, `veyndo_marketing::store::app_channels`,
`veyndo_b2b::store::app_references`, `veyndo_shop::store::app_frontends`,
`app_languages`, `veyndo_identity::store::app_credentials`, `job_members`,
`veyndo_intelligence::store::app_relations`, `veyndo_core::store::outbox_reads`,
`previews`, `tenants`) in the caller's transaction. Owners the app crate may not
depend on are reached through the `Directory` port (`src/ports.rs`), implemented
in the server (`src/ports/apps.rs`) and registered at startup: customer existence
and the preview expiry of the studio.

Two statements still read `tenants` and `shop_environments`: the event delivery
and schedule claims lock the owning shop row or check its status inside one
`FOR UPDATE … SKIP LOCKED` claim, which a separate call cannot reproduce.

## Open JSON

The host reads every field it acts on through typed serde views
(`veyndo_core::document::view`, `lenient`, `merge`) and writes host-owned members
through typed structs. The following documents stay open JSON by contract; the
host reads or writes only the named members and keeps everything else verbatim.

| Place | What stays open | Reason |
| --- | --- | --- |
| `src/domain/draft_document.rs` (`object`, `member`, `copy`, `schema_at`, `schema_node`) | Manifest drafts and the generation JSON Schema | Model output is patched member by member so unknown keys survive until native `Manifest` validation; JSON Schema is a keyword document addressed by path. |
| `src/domain/generation_contract.rs` (`retain`) | The generated draft | Entity, field, surface, action and view entries are located by name/id and their storage/access members copied from the typed prior manifest; other members of each entry are app-declared. |
| `src/domain/generation_edits.rs` (`apply`) | The edited manifest | RFC 6902 paths address arbitrary app-declared keys. |
| `src/domain/generation_brief.rs`, `app_generation.rs` | Brief entity and setting declarations, the bundled manifest schema | Passed through to native `Manifest` validation, which types them. |
| `src/domain/action_inputs.rs` | Action input schemas beyond `properties` | App-declared JSON Schema checked by the native validator. |
| `src/service/flow_apps.rs` (`arguments`) | Flow step arguments | Merchant-declared action input; only the bound host members are written. |
| `src/service/job_evidence.rs`, `record_import.rs` | Record `fields` | App-declared entity fields; only the host-owned `evidence` member is written. |
| `src/service/carrier_booking.rs`, `external_orders.rs`, `store/native_lifecycle.rs` | Stored order and cart documents | The orders/checkout owners define these documents; apps write only `revision`, `order` and `nativeLifecycle` and edit the selected delivery in place. |
| `src/store/surface_registry.rs`, `extension_projection.rs`, `tool_catalogue.rs`, `managed_data.rs`, `planning.rs` | Projected view payloads, record documents and tool descriptors | Built by the vendored app model or app-declared; the host adds `tenant`, `actions`, `id`/`revision`, `extensions`, `_meta`, `ontology`, `elements` or `revision` through typed structs. |
| `src/domain/design_context.rs` (`receipt`) | Reference evidence sources | Untrusted evidence passed on as recorded; only `text` is replaced by its `digest`. |
