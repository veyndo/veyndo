# Component mirrors and their release

The monorepo is the source of truth for every component. Each component repository
is a **generated mirror**: `scripts/component_mirrors.py` produces it from one
monorepo commit, checks that it builds and tests alone, and `components.release.json`
records the commit it was generated from. This is item 5.4 of the
[component architecture plan](plans/2026-11-19-component-architecture.md).

## What a mirror contains

For every component in `components.json` that has a crate of its own, the generator
writes `target/mirrors/<component>/`:

| Part | Source |
| --- | --- |
| The component's crates | its `sources` with a `Cargo.toml` (for `core`: `crates/kernel`, `crates/runtime`, `components/core`), including `ui/` and `migrations/` when present |
| Dependency crates | every crate reached through path dependencies (core, lower components, the app model and authority crates) |
| Outside files | files named by `include_str!`/`include_bytes!`/`include!`, `license-file` or `readme` outside those crates (fixtures, prompts, bundled manifests), plus the few directories a test reads at run time (`RUNTIME_DATA`) |
| Build inputs | `Cargo.lock`, the checked `.sqlx` query cache, `clippy.toml`, `LICENSE` |
| Generated | a workspace `Cargo.toml` whose members and default members are only the component's crates, `.cargo/config.toml` with `SQLX_OFFLINE=true`, `README.md` and `MIRROR.json` (component, source commit, crates, content digest) |

Every copied file keeps its **monorepo-relative path and exact bytes**. That is the
deliberate layout decision: the path dependencies (`../core`, `../../crates/kernel`)
and the many `include_str!("../../../../fixtures/...")` paths stay valid without
editing a single source file, and a mirror can be compared byte for byte with the
commit it came from. The only rewritten manifest is the workspace root, which drops
the server package and every other component from the membership. Dependency crates
join the mirror workspace as path dependencies; `cargo test` at the mirror root runs
only the component's own crates.

The server crate (`src/`) is never part of a mirror; a component that depends on it
cannot be generated (the generator refuses it). So a component's ports are satisfied
in the mirror only by implementations inside the component, normally its test
adapters in `components/<name>/tests/port_adapters.rs`. The check reports, for each
port trait in `src/ports.rs`, whether the mirror implements it in tests, in the
component itself, or not at all (only the server does).

## Check that every mirror stands alone

```bash
python3 scripts/component_mirrors.py --check
```

The check generates every mirror and runs, inside each, `cargo check --all-targets`
and `cargo test`, offline, with the shared target directory `target/mirror-build`
and without `DATABASE_URL`. Generation keeps file modification times, so the shared
target directory reuses the builds of identical dependency crates across mirrors.
Logs go to `target/mirrors/logs/<component>.log`, the summary to
`target/mirrors/report.json`. It fails when a mirror does not build or test alone, or
when `components.release.json` does not name exactly the mirrored components with an
existing commit. Ports without a test adapter are listed as `NEEDS` lines.

It is registered as the tooling suite `testing/component_mirrors_check`, which first
runs negative controls (a server dependency and a path outside the monorepo are
refused; test, component and missing port implementations are told apart).
`--component <name>...` limits generation or the check to some components.

### State on 25 November 2026

All 19 mirrors build and test alone: core, identity, shop, catalog, inventory,
pricing, shipping, checkout, customers, orders, payments, marketing, b2b, pos, apps,
automation, inference, intelligence and studio.

Ports still implemented only by the server (listed by the check):

| Component | Ports | Why not yet |
| --- | --- | --- |
| intelligence | `AgentHost`, `IntakeHost`, `KnowledgeHost` | host facades over agent runs, catalogue intake and knowledge workers; an adapter needs a database-free model of those workers |
| studio | `ImportHost`, `ReleaseHost`, `WorkHost` | host facades over imports, staging releases and agent work |

Not mirrored, because they have no crate of their own: `host` (the server binary),
`protocols` (MCP/UCP adapters still in `src/`) and `connectors` (a separate service
with its own image). `automation` is generated and checked, but not published from
here: `crates/automation` is imported from its own repository at the commit in
`components.lock.json`, so that repository stays its source.

Limits of the check: mirror tests are the components' database-free unit and
adapter tests. Database-backed behaviour stays in the monorepo's registered suites.
Port methods that take a transaction are implemented by the adapters but exercised
only there. The `ui/` folders are copied but build only inside the Veyndo frontend,
which owns the shared kit. The whole `.sqlx` cache is copied, not only the queries a
component uses.

## Where the released commit is recorded

`components.lock.json` stays what it is: the **inbound** lock of vendored components
imported from their own repositories (marketplaces, the app-storage service, the
automation rules), verified by `scripts/component_sources.py` against exact file
hashes. Mirrors go the other way, and their names collide with it (the lock's `apps`
is the `veyndo/apps` app-storage repository; the mirror `apps` is the app platform,
`veyndo-app-platform`). So the **outbound** record is a separate file,
`components.release.json`:

| Field | Meaning |
| --- | --- |
| `commit` | the monorepo commit the mirror was generated from |
| `digest` | SHA-256 over the sorted path and content hashes of the generated mirror (without `MIRROR.json`) |
| `repository` | the component repository the mirror is published to, named after its primary crate |
| `crates`, `dependencies` | the component's crates and the crates copied in with it |
| `published` | the commit in the component repository once published; `null` until then |
| `publish: false`, `reason` | set for a mirror whose upstream is its own repository (`automation`) |
| `not_mirrored` | components without a crate of their own, with the reason |

## Release flow

1. **Generate and check** on the release candidate:
   `python3 scripts/component_mirrors.py --check`. Every mirror must build and test
   alone; review new `NEEDS` lines.
2. **Record** from a clean checkout of the release commit:
   `python3 scripts/component_mirrors.py --record`. It refuses a dirty tree, so the
   recorded commit is the source of every mirror, and writes `components.release.json`.
   Commit that file.
3. **Publish** (not done yet): for each component with `publish` not `false`, replace
   the tree of its repository with `target/mirrors/<component>/` regenerated from the
   recorded commit, commit it with the monorepo commit in the message, tag it with the
   Veyndo version and write that repository commit into `published`. Nobody edits a
   component repository directly; changes return to the monorepo.

Later, published mirrors can depend on each other's released repositories (Git
dependencies at their `published` commits) instead of carrying source copies of their
dependency crates. Until component repositories exist, the copies keep each mirror
reproducible from one monorepo commit.
