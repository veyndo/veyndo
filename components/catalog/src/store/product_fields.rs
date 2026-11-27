//! Writes and reads of the commerce fields of a product; stock is only written when a quantity is given.
use crate::store::exact::Exact;
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
    sqlx::query!(r#"UPDATE products SET price_decimal=$1,tax_rate_decimal=$2,stock=coalesce($3,stock),min_purchase=$4,purchase_steps=$5,max_purchase=$6,delivery_days=$7,list_price_decimal=$8,advanced_prices=$9,media=$10,properties=$11 WHERE tenant=$12 AND id=$13"#, Exact(fields.price) as Exact, Exact(fields.tax_rate) as Exact, fields.stock, fields.min_purchase as i32, fields.purchase_steps as i32, fields.max_purchase.map(|n|n as i32), fields.delivery_days, fields.list_price.map(Exact) as Option<Exact>, json!(fields.advanced_prices), &fields.media, json!(fields.properties), tenant, id).execute(&mut *tx).await?;
    sqlx::query!(
        r#"UPDATE products SET regulation_price_decimal=$1,reference_price=$2 WHERE tenant=$3 AND id=$4"#,
        fields.regulation_price.map(Exact) as Option<Exact>,
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
    json!({"price":r.get::<Exact,_>("price_decimal"),"taxRate":r.get::<Exact,_>("tax_rate_decimal"),"stock":r.get::<i32,_>("stock"),"minPurchase":r.get::<i32,_>("min_purchase"),"purchaseSteps":r.get::<i32,_>("purchase_steps"),"maxPurchase":r.get::<Option<i32>,_>("max_purchase"),"deliveryDays":r.get::<i32,_>("delivery_days"),"listPrice":r.get::<Option<Exact>,_>("list_price_decimal"),"regulationPrice":r.get::<Option<Exact>,_>("regulation_price_decimal"),"referencePrice":r.get::<Option<Value>,_>("reference_price"),"advancedPrices":r.get::<Value,_>("advanced_prices"),"media":r.get::<Value,_>("media"),"properties":r.get::<Value,_>("properties")})
}
