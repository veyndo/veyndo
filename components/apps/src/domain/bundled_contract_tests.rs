//! Bundled app contracts run through the real package compiler: UI programs, native views, assistant fixtures, surfaces and routes reject cross-scope actions, unsafe URLs, smuggled authority and unbound writes.
use crate::domain::{field_values, manifest_validation, native_views};
use serde_json::json;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::Result;

/// The compiler with the operator's default publisher set.
fn validate(m: &Manifest) -> Result<()> {
    manifest_validation::validate(m, &json!({}))
}
mod ui_logic {
    use super::*;
    fn example() -> Manifest {
        let mut m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/care-studio/manifest.json"
        ))
        .unwrap();
        let form = m.views[0]
            .blocks
            .iter()
            .find(|b| b.kind == "form")
            .unwrap()
            .id
            .clone();
        let mut button: veyndo_app_model::views::NativeBlock=serde_json::from_value(json!({"id":"save_button","kind":"button","title":{"en":"Save"},"handlers":{"click":[{"op":"validate","target":form},{"op":"call","action":"save_guides","input":{"kind":"record","block":form}},{"op":"refresh","target":m.views[0].blocks[0].id}]}})).unwrap();
        m.views[0].blocks.push(button.clone());
        button.id = "container".into();
        button.kind = "frame".into();
        button.handlers = None;
        button.child_blocks = vec!["save_button".into()];
        m.views[0].blocks.push(button);
        m
    }
    #[test]
    fn code_behind_runs_only_approved_actions_and_valid_references() {
        let m = example();
        validate(&m).unwrap();
        for change in 0..5 {
            let mut v = json!(m.clone());
            let steps = &mut v["views"][0]["blocks"][2]["handlers"]["click"];
            match change {
                0 => steps[1]["action"] = json!("unknown"),
                1 => steps[1]["input"]["block"] = json!("save_button"),
                2 => v["surfaces"][0]["actions"] = json!(["list_guides"]),
                3 => v["views"][0]["blocks"][3]["childBlocks"] = json!(["container"]),
                _ => {
                    v["views"][0]["blocks"][3]["childBlocks"] =
                        json!(["save_button", "save_button"])
                }
            }
            assert!(
                validate(&serde_json::from_value(v).unwrap()).is_err(),
                "change {change}"
            );
        }
    }
    #[test]
    fn input_literals_do_not_coerce_and_ordered_comparisons_are_typed() {
        let mut m = example();
        m.entities[0]
            .fields
            .push(serde_json::from_value(json!({"name":"quantity","kind":"integer"})).unwrap());
        m.views[0].blocks.push(serde_json::from_value(json!({"id":"qty","kind":"textbox","title":{"en":"Quantity"},"entity":"guides","dataField":"quantity","readAction":"list_guides"})).unwrap());
        let button = m.views[0]
            .blocks
            .iter()
            .position(|b| b.id == "save_button")
            .unwrap();
        m.views[0].blocks[button].handlers=Some(serde_json::from_value(json!({"click":[{"op":"set","target":"qty","value":{"kind":"literal","value":10}},{"op":"if","left":{"kind":"value","block":"qty"},"compare":"gt","right":{"kind":"literal","value":5},"then":[],"otherwise":[]}]})).unwrap());
        validate(&m).unwrap();
        let mut invalid = json!(m.clone());
        invalid["views"][0]["blocks"][button]["handlers"]["click"][0]["value"]["value"] =
            json!("10");
        assert!(validate(&serde_json::from_value(invalid).unwrap()).is_err());
        let mut invalid = json!(m);
        invalid["views"][0]["blocks"][button]["handlers"]["click"][1]["right"]["value"] =
            json!("5");
        assert!(validate(&serde_json::from_value(invalid).unwrap()).is_err());
    }
    #[test]
    fn extracted_ui_compiler_uses_native_field_and_public_read_policies() {
        use veyndo_app_model::{fields::ValuePolicy, views::Policy};
        let policy = field_values::Policy;
        assert!(policy.public_read(false));
        assert!(!policy.public_read(true));
        assert!(policy.money(&json!({"minor":123,"currency":{"code":"JPY","scale":0}})));
        assert!(!policy.money(&json!({"minor":123,"currency":{"code":"JPY","scale":2}})));
        let mut m = example();
        m.entities[0].fields.push(
            serde_json::from_value(json!({"name":"caption","kind":"string","translatable":true}))
                .unwrap(),
        );
        m.views[0].blocks.push(serde_json::from_value(json!({"id":"caption","kind":"textbox","title":{"en":"Caption"},"entity":"guides","dataField":"caption","readAction":"list_guides"})).unwrap());
        let button = m.views[0]
            .blocks
            .iter_mut()
            .find(|b| b.id == "save_button")
            .unwrap();
        button.handlers=Some(serde_json::from_value(json!({"click":[{"op":"set","target":"caption","value":{"kind":"literal","value":{"de-DE":"Bild"}}}]})).unwrap());
        validate(&m).unwrap();
        let mut invalid = json!(m);
        let index = invalid["views"][0]["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .position(|b| b["id"] == "save_button")
            .unwrap();
        invalid["views"][0]["blocks"][index]["handlers"]["click"][0]["value"]["value"] =
            json!({"invalid_locale":"Bild"});
        assert!(validate(&serde_json::from_value(invalid).unwrap()).is_err());
    }
}

mod native_view_contracts {
    use super::*;
    fn example() -> Manifest {
        serde_json::from_str(include_str!(
            "../../../../extensions/apps/care-studio/manifest.json"
        ))
        .unwrap()
    }
    #[test]
    fn care_studio_contract_runs_through_existing_validation() {
        let m = example();
        validate(&m).unwrap();
        assert!(native_views::payload(&m, &m.surfaces[0]).is_some());
    }
    #[test]
    fn native_contract_cannot_smuggle_actions_or_public_forms() {
        for change in 0..7 {
            let mut m = example();
            match change {
                0 => m.views[0].blocks[0].read_action = Some("save_guides".into()),
                1 => m.views[1].blocks[0].kind = "form".into(),
                2 => m.surfaces[0].actions.clear(),
                3 => m.views[0].blocks[0].entity = Some("unknown".into()),
                4 => m.views[0].blocks[0].kind = "javascript".into(),
                5 => m.surfaces[0].ui_path = "native/missing".into(),
                _ => m.views[0].blocks = vec![m.views[0].blocks[0].clone(); 33],
            }
            assert!(validate(&m).is_err(), "mutation {change}");
        }
    }
    #[test]
    fn text_blocks_cannot_bind_an_action_and_read_blocks_cannot_write() {
        let mut m = example();
        m.views[0].blocks[0].kind = "text".into();
        assert!(validate(&m).is_err());
        let mut m = example();
        m.views[0].blocks[0].write_action = Some("save_guides".into());
        assert!(validate(&m).is_err());
    }
    #[test]
    fn public_registry_contains_only_bound_entity_metadata() {
        let mut m = example();
        let mut private = m.entities[0].clone();
        private.name = "private_notes".into();
        private.public_read = false;
        m.entities.push(private);
        let p = native_views::payload(&m, &m.surfaces[1]).unwrap();
        assert_eq!(p["entities"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn empty_native_views_do_not_change_legacy_serialization() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        ))
        .unwrap();
        assert!(json!(m).get("views").is_none());
    }

    #[test]
    fn form_geometry_and_presentation_are_validated_on_the_server() {
        let mut m = example();
        m.views[0].layout = "form".into();
        m.views[0].blocks[0].geometry = Some(veyndo_app_model::form_layout::Geometry {
            x: 0,
            y: 0,
            w: 6,
            h: 6,
        });
        validate(&m).unwrap();
        m.views[0].blocks[0].geometry.as_mut().unwrap().x = 11;
        assert!(validate(&m).is_err());
        m.views[0].blocks[0].geometry.as_mut().unwrap().x = 0;
        m.views[0].blocks[0].tab_order = Some(1001);
        assert!(validate(&m).is_err());
    }
}

mod editor {
    use super::*;
    fn example(name: &str) -> Manifest {
        serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/extensions/apps/assistant-examples/{name}.json",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../..")
            ))
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn every_assistant_builds_a_valid_normal_package() {
        for name in [
            "frontend",
            "admin",
            "combined",
            "payment",
            "shipping",
            "integration",
            "event",
            "webhook",
            "scheduled",
            "customer",
            "order",
            "product_general",
        ] {
            validate(&example(name)).unwrap_or_else(|e| panic!("{name}: {}", e.1));
        }
    }
    #[test]
    fn editor_context_and_private_core_references_cannot_be_smuggled() {
        for mutation in 0..7 {
            let mut m = example("customer");
            match mutation {
                0 => m.entities[0].public_read = true,
                1 => m.entities[0].fields[0].indexed = false,
                2 => m.entities[0].fields[0].core_reference = Some("arbitrary_table".into()),
                3 => m.views[0].blocks[0].context_binding.as_mut().unwrap().key = "orderId".into(),
                4 => m.views[0].blocks[0].context_binding.as_mut().unwrap().field = "title".into(),
                5 => m.entities[0].fields[0].required = false,
                _ => m.surfaces[0].location = "admin.navigation".into(),
            }
            assert!(validate(&m).is_err(), "mutation {mutation}");
        }
    }
    #[test]
    fn choice_values_are_typed_and_checked_at_write_time() {
        let m = example("admin");
        let e = &m.entities[0];
        let mut v = json!({"product_id":"mug","title":{"en":"Care"},"segment":"standard"});
        assert!(field_values::validate(e, &v).is_ok());
        v["segment"] = json!("unlisted");
        assert!(field_values::validate(e, &v).is_err());
        v["segment"] = json!(true);
        assert!(field_values::validate(e, &v).is_err());
        let mut m = m.clone();
        let c = m.entities[0].fields[3].choices[0].clone();
        m.entities[0].fields[3].choices.push(c);
        assert!(validate(&m).is_err());
    }
    #[test]
    fn automation_cannot_invoke_an_arbitrary_mutation() {
        let mut m = example("scheduled");
        m.schedules[0].action = "save_entries".into();
        assert!(validate(&m).is_err());
        let mut m = example("webhook");
        m.webhooks[0].action = "list_entries".into();
        assert!(validate(&m).is_err());
        let mut m = example("payment");
        m.actions[0].public = true;
        assert!(validate(&m).is_err());
    }
}

mod surfaces {
    use super::*;
    #[test]
    fn the_full_app_example_has_valid_surfaces_routes_and_intelligence() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/product-lab/manifest.json"
        ))
        .unwrap();
        validate(&m).unwrap();
        for path in [
            "../escape",
            "https://example.test/x",
            "//foreign",
            "ui?token=x",
            "ui#x",
            "%2e%2e/ui",
        ] {
            let mut broken = m.clone();
            broken.surfaces[0].ui_path = path.into();
            assert!(validate(&broken).is_err());
        }
        let mut broken = m.clone();
        broken.surfaces[1].actions.push("save_entry".into());
        assert!(validate(&broken).is_err());
        let mut broken = m.clone();
        broken.api_routes[0].method = "GET".into();
        broken.api_routes[0].action = "support_received".into();
        assert!(validate(&broken).is_err());
        let mut broken = m.clone();
        broken
            .intelligence
            .as_mut()
            .unwrap()
            .entities
            .push("unknown".into());
        assert!(validate(&broken).is_err());
    }
}

mod packages {
    use super::*;
    use veyndo_app_model::declarations::identifier;
    #[test]
    fn identifiers_cannot_inject_sql() {
        assert!(identifier("engraving"));
        for s in [
            include_str!("../../../../extensions/apps/marketplaces/manifest.json"),
            "x;drop",
            "tenant--",
            "1abc",
            "x.y",
            "é",
        ] {
            assert!(!identifier(s));
        }
    }
    #[test]
    fn order_configurations_are_never_public() {
        let mut m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        ))
        .unwrap();
        m.actions
            .iter_mut()
            .find(|a| a.handler == "configurations")
            .unwrap()
            .public = true;
        assert!(validate(&m).is_err());
    }
    #[test]
    fn executable_package_matches_app_source() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        ))
        .unwrap();
        assert_eq!(
            m.configuration.unwrap().wasm_source,
            include_str!("../../../../extensions/apps/engraving/configuration.wat")
        );
    }
    #[test]
    fn additive_action_metadata_does_not_change_published_legacy_digests() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        ))
        .unwrap();
        let value = json!(m);
        assert!(
            value["actions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|a| a.get("readOnly").is_none()
                    && a.get("permission").is_none()
                    && a.get("outputSchema").is_none())
        );
    }
    #[test]
    fn built_in_packages_are_valid() {
        for s in [
            include_str!("../../../../extensions/apps/algolia/manifest.json"),
            include_str!("../../../../extensions/apps/care-studio/manifest.json"),
            include_str!("../../../../extensions/apps/catalog-export/manifest.json"),
            include_str!("../../../../extensions/apps/contentful/manifest.json"),
            include_str!("../../../../extensions/apps/edi/manifest.json"),
            include_str!("../../../../extensions/apps/email/manifest.json"),
            include_str!("../../../../extensions/apps/engraving/manifest.json"),
            include_str!("../../../../extensions/apps/erp-handoff/manifest.json"),
            include_str!("../../../../extensions/apps/furniture-configurator/manifest.json"),
            include_str!("../../../../extensions/apps/gift-message/manifest.json"),
            include_str!("../../../../extensions/apps/gmail/manifest.json"),
            include_str!("../../../../extensions/apps/google-analytics/manifest.json"),
            include_str!("../../../../extensions/apps/hubspot/manifest.json"),
            include_str!("../../../../extensions/apps/klaviyo/manifest.json"),
            include_str!("../../../../extensions/apps/marketplaces/manifest.json"),
            include_str!("../../../../extensions/apps/migration/manifest.json"),
            include_str!("../../../../extensions/apps/object-context/manifest.json"),
            include_str!("../../../../extensions/apps/ontology-care/manifest.json"),
            include_str!("../../../../extensions/apps/order-alerts/manifest.json"),
            include_str!("../../../../extensions/apps/packing-helper/manifest.json"),
            include_str!("../../../../extensions/apps/payment-provider/manifest.json"),
            include_str!("../../../../extensions/apps/paypal/manifest.json"),
            include_str!("../../../../extensions/apps/product-advisor/manifest.json"),
            include_str!("../../../../extensions/apps/product-lab/manifest.json"),
            include_str!("../../../../extensions/apps/production-review/manifest.json"),
            include_str!("../../../../extensions/apps/returns-workspace/manifest.json"),
            include_str!("../../../../extensions/apps/service-example/manifest.json"),
            include_str!("../../../../extensions/apps/shopware-payments/manifest.json"),
            include_str!("../../../../extensions/apps/site-builder/manifest.json"),
            include_str!("../../../../extensions/apps/site-forms/manifest.json"),
            include_str!("../../../../extensions/apps/slack/manifest.json"),
            include_str!("../../../../extensions/apps/snapshot-pricing/manifest.json"),
            include_str!("../../../../extensions/apps/storyfront/manifest.json"),
            include_str!("../../../../extensions/apps/vercel/manifest.json"),
        ] {
            validate(&serde_json::from_str(s).unwrap()).unwrap();
        }
    }
}

mod field_contracts {
    use super::*;
    use std::collections::HashMap;
    use veyndo_app_model::{Entity, Field};
    #[test]
    fn fields_reject_type_and_unknown_properties() {
        let e = Entity {
            operational: None,
            extends: None,
            name: "config".into(),
            label: HashMap::new(),
            public_read: false,
            publication: None,
            workflow: None,
            fields: vec![Field {
                name: "fee".into(),
                translatable: false,
                label: HashMap::new(),
                kind: "integer".into(),
                required: true,
                indexed: false,
                references: None,
                core_reference: None,
                choices: vec![],
                unique: false,
                validation: None,
            }],
        };
        assert!(field_values::validate(&e, &json!({"fee":200})).is_ok());
        assert!(field_values::validate(&e, &json!({"fee":"200"})).is_err());
        assert!(field_values::validate(&e, &json!({"fee":200,"tenant":"other"})).is_err());
    }
}
