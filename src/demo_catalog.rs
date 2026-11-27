//! Public synthetic fashion template; provisioning copies only this versioned fixture into a new tenant.
use crate::catalog_model::prototype_products;
use crate::foundation::App;
use crate::store::shops;
use serde_json::Value;
use serde_json::json;
use std::env;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_intelligence::knowledge;

pub(crate) fn template() -> Value {
    serde_json::from_str(include_str!("../fixtures/fashion-catalog.json"))
        .expect("valid bundled fashion catalogue")
}

pub(crate) async fn insert(tx: &mut sqlx::PgConnection, tenant: &str) -> Result<()> {
    let source = match env::var("DEMO_CATALOG").as_deref() {
        Ok("legacy-furniture") => {
            serde_json::from_str(include_str!("../fixtures/demo-catalog.json")).unwrap()
        }
        Ok("fashion") | Err(_) => template(),
        Ok(_) => return Err(bad("Unsupported demo catalogue")),
    };
    veyndo_catalog::store::demo_template::insert_template(tx, tenant, source).await
}

/// An opt-in built-in demo is added once; subsequent starts never replace merchant edits or orders.
pub(crate) async fn seed_builtin(a: &App) -> Result<()> {
    let mut tx = a.db.begin().await?;
    if !shops::create(&mut tx, "nord-atelier", "Nord Atelier").await? {
        return Ok(());
    }
    veyndo_catalog::store::demo_template::insert_template(&mut tx, "nord-atelier", template())
        .await?;
    let settings: Value =
        serde_json::from_str(include_str!("../fixtures/demo-settings.json")).unwrap();
    shops::insert_settings(&mut tx, "nord-atelier", settings).await?;
    shops::seed_automation(&mut *tx, "nord-atelier").await?;
    crate::categories::seed(&mut tx, "nord-atelier").await?;
    categories(&mut tx, "nord-atelier").await?;
    shops::insert_experience(
        &mut *tx,
        "nord-atelier",
        json!({"mode":"balanced","headline":"A considered wardrobe. Made for everyday."}),
        false,
    )
    .await?;
    for variant in ["discovery", "comparison"] {
        shops::insert_policy(&mut *tx, "nord-atelier", variant, false).await?;
    }
    tx.commit().await?;
    for p in prototype_products(a, "nord-atelier").await? {
        knowledge::sync_product_scoped(&a.db, "nord-atelier", &json!(p)).await?;
    }
    knowledge::seed_relations(&a.db, "nord-atelier").await?;
    Ok(())
}

pub(crate) async fn categories(tx: &mut sqlx::PgConnection, tenant: &str) -> Result<()> {
    veyndo_catalog::store::demo_template::translate_categories(tx, tenant, &template()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fashion_template_has_complete_localized_products_and_real_asset_files() {
        let data = template();
        let products = data["products"].as_array().unwrap();
        let roots: Vec<_> = products
            .iter()
            .filter(|p| p["parent_id"].is_null())
            .collect();
        assert_eq!(roots.len(), 12);
        let ids: std::collections::HashSet<_> =
            products.iter().map(|p| p["id"].as_str().unwrap()).collect();
        assert_eq!(ids.len(), products.len());
        for p in products {
            assert!(p["price"].as_f64().unwrap() > 0.);
            if let Some(parent) = p["parent_id"].as_str() {
                let root = roots.iter().find(|r| r["id"] == parent).unwrap();
                // Cart, order and POS labels append the options; a size in the
                // variant name would read "Coat · L · camel / L".
                assert_eq!(
                    p["name"], root["name"],
                    "variant {} repeats its options",
                    p["id"]
                );
                for tr in data["translations"].as_array().unwrap() {
                    if tr["product_id"] == p["id"] {
                        assert!(data["translations"].as_array().unwrap().iter().any(
                            |r| r["product_id"] == parent
                                && r["language_id"] == tr["language_id"]
                                && r["name"] == tr["name"]
                        ));
                    }
                }
            }
            for media in p["media"].as_array().unwrap() {
                let url = media["url"].as_str().unwrap();
                assert!(url.starts_with("/media/demo/fashion/") && url.ends_with(".webp"));
                let bytes = std::fs::read(format!("frontend/public{url}")).unwrap();
                assert_eq!(&bytes[..4], b"RIFF");
                assert_eq!(&bytes[8..12], b"WEBP");
            }
            for language in [
                "2fbb5fe2e29a4d70aa5854ce7ce3e20b",
                "11111111111111111111111111111111",
                "22222222222222222222222222222222",
                "33333333333333333333333333333333",
            ] {
                assert!(
                    data["translations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|tr| tr["product_id"] == p["id"]
                            && tr["language_id"] == language
                            && !tr["name"].as_str().unwrap().is_empty())
                );
            }
        }
    }

    /// Every colour and the one-size value of the fashion catalogue are named in en/de/fr/es on the
    /// family root, where `extra.optionValues` is read for the whole family.
    #[test]
    fn fashion_option_values_are_named_in_four_languages() {
        let data = template();
        let products = data["products"].as_array().unwrap();
        for p in products {
            let root_id = p["parent_id"].as_str().unwrap_or(p["id"].as_str().unwrap());
            let root = products.iter().find(|r| r["id"] == root_id).unwrap();
            assert!(p["parent_id"].is_null() || p["extra"].get("optionValues").is_none());
            for (group, value) in p["options"].as_object().unwrap() {
                let value = value.as_str().unwrap();
                if group == "size" && value != "one-size" {
                    continue; // S, M, L and shoe sizes read the same in every language.
                }
                let names = &root["extra"]["optionValues"][group][value];
                for language in ["en", "de", "fr", "es"] {
                    assert!(
                        names[language].as_str().is_some_and(|s| !s.is_empty()),
                        "{root_id} {group}={value} lacks {language}"
                    );
                }
            }
        }
    }
}
