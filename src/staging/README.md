# src/staging

This folder owns the Rust modules listed below. Each source begins with its responsibility contract. The crate currently shares internal types/imports through a facade; APIs, MCP and UCP delegate to shared domain operations.

- [`final_config.rs`](../../components/studio/src/service/staging/final_config.rs): Validate final channel/Flow settings and initialize native POS defaults without another payment owner.
- [`clone.rs`](../../components/studio/src/service/staging/clone.rs): Clone only catalog/configuration into a private tenant; customer/order/payment state is excluded.
- [`documents.rs`](../../components/intelligence/src/store/documents.rs): Knowledge documents/chunks clone and publish with their source provenance; publication visibility is a reviewed unit.
- [`mod.rs`](mod.rs): Private cloned shops, scope admission and selective atomic release of reviewed changes.
- [`release.rs`](../../components/studio/src/service/staging/release.rs): Selected units publish in one transaction with staged digests and live baseline conflict checks.
- [`snapshot.rs`](../../components/studio/src/service/staging/snapshot.rs): Fixed publishable units: product content/translations, settings, experience and app packages.

The [source inventory](../../docs/module-inventory.md) is checked in CI. [The behavioral map](../../docs/source-map.md) identifies integration suites, and [testing](../../docs/testing.md) describes actual coverage and limits. Every file is limited to 320 lines; `main.rs` to 120.

`company.rs` adds independent `company` / `company-channel:{id}` release units with linked logo copying, digest deduplication, validation and a canonical live baseline.

B2B configuration/catalog units delegate to `b2b/staging.rs`; employees, financial decisions and observations remain excluded. Private B2B diffs require current read/PII rights.
