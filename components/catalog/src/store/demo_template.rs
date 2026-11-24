//! Demo template rows: the products and translations of a bundled catalogue template, and the template's category translations.
use serde_json::{Value, json};
use veyndo_core::Result;

/// Inserts every product and translation of the template into a new shop.
pub async fn insert_template(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    template: Value,
) -> Result<()> {
    for p in template["products"].as_array().unwrap() {
        sqlx::query("INSERT INTO products(tenant,id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra) VALUES($1,$2,$3,$4,$5,$6,$7,$8,1,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)")
            .bind(tenant).bind(p["id"].as_str()).bind(p["name"].as_str()).bind(p["category"].as_str()).bind(p["description"].as_str())
            .bind(p["price"].as_f64()).bind(p["tax_rate"].as_f64()).bind(p["stock"].as_i64().unwrap() as i32)
            .bind(p["list_price"].as_f64()).bind(p["regulation_price"].as_f64()).bind(p["reference_price"].as_object().map(|_|p["reference_price"].clone()))
            .bind(&p["advanced_prices"]).bind(p["min_purchase"].as_i64().unwrap() as i32).bind(p["purchase_steps"].as_i64().unwrap() as i32)
            .bind(p["max_purchase"].as_i64().map(|v|v as i32)).bind(p["parent_id"].as_str()).bind(&p["options"]).bind(&p["media"]).bind(&p["properties"])
            .bind(p["delivery_days"].as_i64().unwrap() as i32).bind(p.get("extra").cloned().unwrap_or(json!({}))).execute(&mut *tx).await?;
    }
    for tr in template["translations"].as_array().unwrap() {
        sqlx::query("INSERT INTO product_translations(tenant,product_id,language_id,name,description) VALUES($1,$2,$3,$4,$5)")
            .bind(tenant).bind(tr["product_id"].as_str()).bind(tr["language_id"].as_str()).bind(tr["name"].as_str()).bind(tr["description"].as_str()).execute(&mut *tx).await?;
    }
    Ok(())
}

/// Gives the seeded legacy categories the translations of the template.
pub async fn translate_categories(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    template: &Value,
) -> Result<()> {
    for category in template["categories"].as_array().into_iter().flatten() {
        sqlx::query("UPDATE categories SET data=jsonb_set(data,'{translations}',$3) WHERE tenant=$1 AND id='legacy-'||md5($2)")
            .bind(tenant).bind(category["id"].as_str()).bind(&category["translations"]).execute(&mut *tx).await?;
    }
    Ok(())
}

/// Inserts one minimal product row of the legacy furniture demo; an existing row is kept.
pub async fn insert_basic(
    a: &veyndo_core::State,
    tenant: &str,
    (id, name, category, description): (&str, &str, &str, &str),
    (price, tax, stock): (f64, f64, i32),
) -> Result<()> {
    sqlx::query("INSERT INTO products(tenant,id,name,category,description,price,tax_rate,stock) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING").bind(tenant).bind(id).bind(name).bind(category).bind(description).bind(price).bind(tax).bind(stock).execute(&a.db).await?;
    Ok(())
}
