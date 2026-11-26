# src/accounts

This folder owns the Rust modules listed below. Each source begins with its responsibility contract. The crate currently shares internal types/imports through a facade; APIs, MCP and UCP delegate to shared domain operations.

- [`address_restore.rs`](../../components/customers/src/store/address_restore.rs): Atomic restoration of the owning customer's address aggregate; geography, references and immutable order snapshots remain guarded.
- [`address_store.rs`](address_store.rs): Tenant-owned address persistence, optimistic revisions and atomic default assignment.
- [`contacts.rs`](../../extensions/apps/migration/adapters/contacts.rs): Customer-editable contact fields; identity, price group and privileges remain server-owned.
- [`demo.rs`](demo.rs): Public synthetic customer fixture for newly provisioned demo shops; never fills real customer addresses.
- [`mod.rs`](mod.rs): Independent customer sessions, profile/password management and owning-account order history.

The [source inventory](../../docs/module-inventory.md) is checked in CI. [The behavioral map](../../docs/source-map.md) identifies integration suites, and [testing](../../docs/testing.md) describes actual coverage and limits. Every file is limited to 320 lines; `main.rs` to 120.

`address_restore.rs` rebuilds the owning customer address book through current geography/ownership checks. Account history excludes credentials. See [entity history](../../docs/entity-history.md).
