//! Bulk test stock for a private sandbox: empty purchasable products get a quantity through the native revision, index sync and event.
use crate::*;
const BATCH: i64 = 500;
pub(crate) async fn fill(a: &App, t: &str, quantity: i32) -> Result<Value> {
    if !(1..=1000).contains(&quantity) {
        return Err(bad("Test stock must be between 1 and 1000"));
    }
    let mut tx = a.db.begin().await?;
    // Stock is operational state: only a private sandbox may receive invented quantities.
    let sandbox: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NULL)",
    )
    .bind(t)
    .fetch_one(&mut *tx)
    .await?;
    if !sandbox {
        return Err(conflict("Test stock can only be set in a private sandbox"));
    }
    let rows = sqlx::query("UPDATE products p SET stock=$2,revision=revision+1 WHERE tenant=$1 AND id IN (SELECT id FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id) ORDER BY e.id LIMIT $3 FOR UPDATE) RETURNING *")
        .bind(t).bind(quantity).bind(BATCH).fetch_all(&mut *tx).await?;
    for row in &rows {
        knowledge::sync_product(&mut tx, t, &json!(product(row))).await?;
        sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'product.updated',$2)")
            .bind(t)
            .bind(json!({"productId":row.get::<String,_>("id"),"revision":row.get::<i64,_>("revision")}))
            .execute(&mut *tx)
            .await?;
    }
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id)")
        .bind(t).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(json!({"updated":rows.len(),"quantity":quantity,"remaining":remaining,"sandboxOnly":true}))
}
