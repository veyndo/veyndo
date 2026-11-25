//! The server's real port implementations against the contract sets of `veyndo-port-contracts`; the test adapters there pass the same sets.
use veyndo_port_contracts::{public_egress, record_rules, settings_guards};

#[test]
fn settings_guards_meet_the_contract() {
    settings_guards::condition(&crate::commerce::Guards);
}

#[tokio::test]
async fn public_egress_meets_the_contract() {
    public_egress::refuses_private_destinations(&crate::assets::Egress).await;
}

#[test]
fn bundled_record_rules_meet_the_contract() {
    let site: veyndo_app_model::manifest::Manifest = serde_json::from_str(include_str!(
        "../extensions/apps/site-builder/manifest.json"
    ))
    .expect("bundled site builder manifest");
    let pages = site
        .entities
        .iter()
        .find(|e| e.name == "pages")
        .expect("pages entity");
    record_rules::pure_rules(
        &crate::apps::data::BUNDLED,
        &record_rules::plain_manifest(),
        Some(record_rules::Ruled {
            manifest: &site,
            entity: pages,
            refused: serde_json::json!({"title": 5, "blocks": "not a list"}),
        }),
    );
}
