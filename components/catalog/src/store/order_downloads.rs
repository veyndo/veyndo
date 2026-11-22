//! Order downloads: a purchased digital product (or its parent) grants its published download assets to the order.
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use veyndo_core::{Result, bad};

/// Grants each digital line's published downloads; a digital product without one refuses the order.
pub async fn grant(
    tx: &mut PgConnection,
    tenant: &str,
    products: &[String],
    order: &str,
) -> Result<()> {
    for product in products {
        let r = sqlx::query("SELECT id,parent_id,extra FROM products WHERE tenant=$1 AND id=$2")
            .bind(tenant)
            .bind(product)
            .fetch_one(&mut *tx)
            .await?;
        let extra: Value = r.get("extra");
        let parent: Option<String> = r.get("parent_id");
        let parent_extra: Value = if let Some(parent) = &parent {
            sqlx::query_scalar("SELECT extra FROM products WHERE tenant=$1 AND id=$2")
                .bind(tenant)
                .bind(parent)
                .fetch_one(&mut *tx)
                .await?
        } else {
            json!({})
        };
        let digital = extra.get("digital").unwrap_or(&parent_extra["digital"]) == true;
        if digital {
            let n=sqlx::query("INSERT INTO order_downloads(tenant,order_id,asset_id) SELECT tenant,$1,id FROM product_assets WHERE tenant=$2 AND product_id=ANY($3) AND kind='download' AND public ON CONFLICT DO NOTHING").bind(order).bind(tenant).bind(vec![product.clone(),parent.unwrap_or_default()]).execute(&mut *tx).await?.rows_affected();
            if n == 0 {
                return Err(bad("Digital product has no published download"));
            }
        }
    }
    Ok(())
}
