//! The server's own SQL: shop provisioning across components, the operator control plane, the migration runner and process-level checks. Components keep theirs in their own store modules.
pub(crate) mod migrations;
pub(crate) mod platform;
pub(crate) mod process;
pub(crate) mod shops;
