# Studio component

Merchant workspace services: product, order and customer editors, grid plans,
reviewed work and proposals, imports and migration, history and restoration,
reports, the translation worker and private sandbox shops (clone, diff, release).
Module ownership is listed in [the source map](../../docs/source-map.md); the
component boundary and its declared foreign tables are in `components.json`.

## Open JSON

The component reads and writes fields through typed views (`veyndo_core::document::view`),
typed `Serialize` documents or `veyndo_core::document::merge`. These values stay JSON
by contract:

| Place | Reason |
| --- | --- |
| Sandbox snapshots and the baseline (`service/staging.rs`, `service/staging/*`) | A snapshot maps release unit keys (`settings`, `channel:*`, `flow:*`, `appdata:*`, …) to the stored document of that unit's owner. Units are compared, hashed and published whole and looked up by runtime key; `Baseline` types only its `stage`/`live` sides. |
| History versions (`domain/history_access.rs`, `domain/product_restore.rs`) | A recorded version is the snapshot of an arbitrary entity at that time. Restoration reads the product fields it restores through typed views and writes the editor document through `merge`. |
| Operation input schemas (`domain/history_access.rs`, `service/imports.rs`, `domain/grid_criteria.rs`) | JSON Schema documents handed to the model contract and MCP; grid filters are built from a typed field list. |
| Import source records (`service/imports/*`) | `SourceRecord.data` is the foreign shop's record before mapping; mappers read it through typed views. |
| Editor and import documents passed to native owners (`http/product_editor.rs`, `service/imports/products.rs`) | The product owner (catalog) admits the document; Studio only adds the fields it owns. |
