//! Writes and reads of the commerce fields of a product; stock is only written when a quantity is given.
use crate::types::product_fields::ProductFields;
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use veyndo_core::Result;

/// Stores the fields; `stock: None` keeps the stored quantity.
pub async fn save(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    fields: &ProductFields,
) -> Result<()> {
    sqlx::query!(r#"UPDATE products SET price=$1,tax_rate=$2,stock=coalesce($3,stock),min_purchase=$4,purchase_steps=$5,max_purchase=$6,delivery_days=$7,list_price=$8,advanced_prices=$9,media=$10,properties=$11 WHERE tenant=$12 AND id=$13"#, fields.price, fields.tax_rate, fields.stock, fields.min_purchase as i32, fields.purchase_steps as i32, fields.max_purchase.map(|n|n as i32), fields.delivery_days, fields.list_price, json!(fields.advanced_prices), &fields.media, json!(fields.properties), tenant, id).execute(&mut *tx).await?;
    sqlx::query!(
        r#"UPDATE products SET regulation_price=$1,reference_price=$2 WHERE tenant=$3 AND id=$4"#,
        fields.regulation_price,
        fields.reference_price.as_ref().map(|p| json!(p)),
        tenant,
        id
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// The commerce fields of a product row in the shape the editor reads.
pub fn editable(r: &PgRow) -> Value {
    json!({"price":r.get::<f64,_>("price"),"taxRate":r.get::<f64,_>("tax_rate"),"stock":r.get::<i32,_>("stock"),"minPurchase":r.get::<i32,_>("min_purchase"),"purchaseSteps":r.get::<i32,_>("purchase_steps"),"maxPurchase":r.get::<Option<i32>,_>("max_purchase"),"deliveryDays":r.get::<i32,_>("delivery_days"),"listPrice":r.get::<Option<f64>,_>("list_price"),"regulationPrice":r.get::<Option<f64>,_>("regulation_price"),"referencePrice":r.get::<Option<Value>,_>("reference_price"),"advancedPrices":r.get::<Value,_>("advanced_prices"),"media":r.get::<Value,_>("media"),"properties":r.get::<Value,_>("properties")})
}
