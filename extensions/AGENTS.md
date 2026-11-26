# extensions ownership

Root AGENTS.md also applies. Rules below were moved from the root, not weakened.

## Connector service schema

The connector tables are created by `extensions/services/connectors/migrations/0001-baseline.sql`,
which the commerce setup applies in the global baseline order; the connector binary
only starts once the ledger records `connectors/0001-baseline`. There is no import of
state from the earlier SQLite/Python connector worker; do not add one.

## App lifecycle and independent reference services

Native record lifecycle declarations are generic Manifest contracts implemented
by `components/apps/src/domain/record_lifecycle.rs`; application-specific states/edges belong only
to their package. AI edits must preserve existing lifecycle/identity constraints.
Record writes must retain the installed-package advisory fence and transactional
digest check; do not replace it with cached approval or a second write API.
Network admission must use `apps/service_limits.rs` across every app egress lane,
with the tenant aggregate and database leases retained across replicas.

`extensions/apps/erp-handoff/service/` owns its independent Rust process, receipt
schema, provider protocol and configuration; its app-local tests are invoked by
registered `scripts/erp_handoff.py`. `extensions/sdk/rust-events.rs` is generic
wire verification. Do not move ERP/returns code or app receipt tables to core
system directories. Do not implement automatic retries for ambiguous ERP effects
or give the service a core-owner/RLS-bypass database role. Native preview/staging
continues to block external effects. Existing mapping schemas require explicit
owned-order migration before making the new order reference required.

## Runtime extensions and ports

1. Identify the existing module, API/app contract, event producer and actual consumer.
2. Extend the Rust owner while keeping API, MCP, flow and frontend contracts aligned.
   External apps can keep their own language; do not pull them into the public core.
3. There is no data from earlier releases to carry over; do not add import, backfill or
   cutover paths for it. Never run two dispatchers against the same workload.
4. Verify the real path with isolated PostgreSQL tenants and local provider fixtures.
   Use the registered `rust_connectors` / `email_tests` suites when changing these services;
   preserve the original comparison fixtures, concurrency checks and `schema_baseline`.
5. Update module ownership and service documentation. Check the final runtime image
   and startup path, not merely whether Rust code exists or unit tests pass.
