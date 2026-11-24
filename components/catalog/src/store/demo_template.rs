//! Demo template rows: the products and translations of a bundled catalogue template, and the template's category translations.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::{Result, bad};

/// A bundled catalogue template: products, translations and category translations.
#[derive(Deserialize)]
pub struct Template {
    products: Vec<TemplateProduct>,
    translations: Vec<TemplateTranslation>,
    #[serde(default)]
    categories: Vec<TemplateCategory>,
}

/// One product row of a template, in the stored column names.
#[derive(Deserialize)]
struct TemplateProduct {
    id: Option<String>,
    name: Option<String>,
    category: Option<String>,
    description: Option<String>,
    price: Option<f64>,
    tax_rate: Option<f64>,
    stock: i32,
    list_price: Option<f64>,
    regulation_price: Option<f64>,
    #[serde(default)]
    reference_price: Value,
    #[serde(default)]
    advanced_prices: Value,
    min_purchase: i32,
    purchase_steps: i32,
    max_purchase: Option<i32>,
    parent_id: Option<String>,
    #[serde(default)]
    options: Value,
    #[serde(default)]
    media: Value,
    #[serde(default)]
    properties: Value,
    delivery_days: i32,
    extra: Option<Value>,
}

/// One product translation of a template.
#[derive(Deserialize)]
struct TemplateTranslation {
    product_id: Option<String>,
    language_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
}

/// The translations of one template category.
#[derive(Deserialize)]
struct TemplateCategory {
    id: Option<String>,
    #[serde(default)]
    translations: Value,
}

/// Reads a bundled template; a malformed template is an error, never a partial shop.
pub fn parse(template: Value) -> Result<Template> {
    serde_json::from_value(template).map_err(|_| bad("Invalid bundled catalogue template"))
}

/// Inserts every product and translation of the template into a new shop.
pub async fn insert_template(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    template: Value,
) -> Result<()> {
    let template = parse(template)?;
    for p in &template.products {
        sqlx::query("INSERT INTO products(tenant,id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra) VALUES($1,$2,$3,$4,$5,$6,$7,$8,1,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)")
            .bind(tenant).bind(&p.id).bind(&p.name).bind(&p.category).bind(&p.description)
            .bind(p.price).bind(p.tax_rate).bind(p.stock)
            .bind(p.list_price).bind(p.regulation_price).bind(p.reference_price.is_object().then(|| p.reference_price.clone()))
            .bind(&p.advanced_prices).bind(p.min_purchase).bind(p.purchase_steps)
            .bind(p.max_purchase).bind(&p.parent_id).bind(&p.options).bind(&p.media).bind(&p.properties)
            .bind(p.delivery_days).bind(p.extra.clone().unwrap_or(json!({}))).execute(&mut *tx).await?;
    }
    for tr in &template.translations {
        sqlx::query("INSERT INTO product_translations(tenant,product_id,language_id,name,description) VALUES($1,$2,$3,$4,$5)")
            .bind(tenant).bind(&tr.product_id).bind(&tr.language_id).bind(&tr.name).bind(&tr.description).execute(&mut *tx).await?;
    }
    Ok(())
}

/// Gives the seeded legacy categories the translations of the template.
pub async fn translate_categories(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    template: &Value,
) -> Result<()> {
    for category in parse(template.clone())?.categories {
        sqlx::query("UPDATE categories SET data=jsonb_set(data,'{translations}',$3) WHERE tenant=$1 AND id='legacy-'||md5($2)")
            .bind(tenant).bind(&category.id).bind(&category.translations).execute(&mut *tx).await?;
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

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_json::json;

    #[test]
    fn bundled_templates_parse_and_malformed_ones_are_refused() {
        for raw in [
            include_str!("../../../../fixtures/fashion-catalog.json"),
            include_str!("../../../../fixtures/demo-catalog.json"),
        ] {
            let template = parse(serde_json::from_str(raw).unwrap()).unwrap();
            assert!(!template.products.is_empty());
        }
        assert!(parse(json!({"products":[{"id":"x"}],"translations":[]})).is_err());
        assert!(parse(json!({"products":"x","translations":[]})).is_err());
    }
}
