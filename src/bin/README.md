# src/bin

This folder owns the binary entry points of the root package. Each source begins with its responsibility contract.

- [`connectors.rs`](connectors.rs): Independent Rust standard-app process; calls the `veyndo-connectors` service crate for E-mail, Gmail, Analytics and Slack.
- [`verified_kernel/`](verified_kernel/README.md): Generated conformance driver of the `formal_differential` suite; invokes the same production rule functions as the commerce server.

The [source inventory](../../docs/module-inventory.md) is checked in CI. [The source map](../../docs/source-map.md) identifies integration suites, and [testing](../../docs/testing.md) describes actual coverage and limits.
