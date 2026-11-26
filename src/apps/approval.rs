//! Operator service authority is bound to immutable package content, never to a merchant-chosen ID; the rules live in the apps component.
use super::Manifest;
use std::env;
pub(super) use veyndo_app_platform::domain::approval::{approved, built_in, canonical_digest};
use veyndo_core::Result;
pub(super) fn installation(m: &Manifest) -> Result<()> {
    veyndo_app_platform::domain::approval::installation(m, &crate::runtime_config::get().services)
}

pub(crate) fn print_digest() {
    let path = env::args()
        .nth(2)
        .expect("Usage: veyndo --app-digest MANIFEST.json");
    let source = std::fs::read_to_string(path).expect("Read app manifest");
    let m: Manifest = serde_json::from_str(&source).expect("Typed app manifest");
    println!("{}", canonical_digest(&m));
}
