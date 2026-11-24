//! Public synthetic fashion template; provisioning copies only this versioned fixture into a new tenant.
use crate::{App, Result, Value, bad, env, json, knowledge, prototype_products};

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
    let created = sqlx::query(
        "INSERT INTO tenants(id,name) VALUES('nord-atelier','Nord Atelier') ON CONFLICT DO NOTHING",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if created == 0 {
        return Ok(());
    }
    veyndo_catalog::store::demo_template::insert_template(&mut tx, "nord-atelier", template())
        .await?;
    let settings: Value =
        serde_json::from_str(include_str!("../fixtures/demo-settings.json")).unwrap();
    sqlx::query("INSERT INTO commerce_settings(tenant,data) VALUES('nord-atelier',$1)")
        .bind(settings)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT public.seed_shop_automation('nord-atelier')")
        .execute(&mut *tx)
        .await?;
    crate::categories::seed(&mut tx, "nord-atelier").await?;
    categories(&mut tx, "nord-atelier").await?;
    sqlx::query("INSERT INTO experiences(tenant,data) VALUES('nord-atelier',$1)")
        .bind(json!({"mode":"balanced","headline":"A considered wardrobe. Made for everyday."}))
        .execute(&mut *tx)
        .await?;
    for variant in ["discovery", "comparison"] {
        sqlx::query("INSERT INTO policy(tenant,variant) VALUES('nord-atelier',$1)")
            .bind(variant)
            .execute(&mut *tx)
            .await?;
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
                assert!(roots.iter().any(|r| r["id"] == parent));
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
}
