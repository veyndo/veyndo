//! Bulk test stock for a private sandbox: empty purchasable products get a quantity through the native revision, index sync and event.
use crate::ports::Inventory;
use crate::store::test_stock;
use crate::types::events::TestStockFilled;
use serde_json::{Value, json};
use veyndo_core::{Result, bad, conflict};

const BATCH: i64 = 500;

pub async fn fill(i: &Inventory, tenant: &str, quantity: i32) -> Result<Value> {
    if !(1..=1000).contains(&quantity) {
        return Err(bad("Test stock must be between 1 and 1000"));
    }
    let mut tx = i.core.db.begin().await?;
    // Stock is operational state: only a private sandbox may receive invented quantities.
    if !i.environments.is_private_sandbox(&mut tx, tenant).await? {
        return Err(conflict("Test stock can only be set in a private sandbox"));
    }
    let filled = test_stock::fill(&mut tx, tenant, quantity, BATCH).await?;
    for row in &filled {
        i.index
            .product_changed(&mut tx, tenant, &row.product)
            .await?;
        let filled = TestStockFilled {
            product_id: &row.product.id,
            revision: row.revision,
        };
        veyndo_core::events::emit(&mut *tx, tenant, &filled).await?;
    }
    let remaining = test_stock::remaining(&mut tx, tenant).await?;
    tx.commit().await?;
    Ok(json!({"updated":filled.len(),"quantity":quantity,"remaining":remaining,"sandboxOnly":true}))
}
