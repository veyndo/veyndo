//! Orders and receipts as their owning customer account reads them; matching a guest's email never grants access.
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, Row};
use veyndo_core::{Error, Result, State, StatusCode};

/// One immutable account binding for order history, receipts and purchased files.
/// `$2` is the authenticated account email; it resolves its tenant-owned stable ID.
/// Matching a legacy guest's contact email alone never establishes ownership.
pub const ACCOUNT_SCOPE: &str = "(o.data->'orderCustomer'->>'customerId')=(SELECT id FROM customers WHERE tenant=o.tenant AND email=$2)";

/// Raw orders of the account with their creation time, newest first: one by `id`, or a page of 101 after a cursor.
pub async fn owned(
    a: &State,
    t: &str,
    email: &str,
    id: Option<&str>,
    after: Option<&str>,
) -> Result<Vec<(Value, String)>> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT o.data, o.created_at::text AS time FROM orders o JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE o.tenant=$1 AND ($3::text IS NULL OR o.id=$3) AND ",
    );
    query.push(ACCOUNT_SCOPE).push(" AND ($4::text IS NULL OR (o.created_at,o.id)<(SELECT created_at,id FROM orders WHERE tenant=$1 AND id=$4)) ORDER BY o.created_at DESC,o.id DESC LIMIT 101");
    let rows = query
        .build()
        .bind(t)
        .bind(email)
        .bind(id)
        .bind(after)
        .fetch_all(&a.db)
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get("data"), r.get::<String, _>("time")))
        .collect())
}

/// The receipts of an order the customer may see.
pub async fn receipts(a: &State, t: &str, id: &str) -> Result<Vec<Value>> {
    let rows = sqlx::query("SELECT id,kind,number,locale,created_at::text AS created FROM order_receipts WHERE tenant=$1 AND order_id=$2 AND customer_visible ORDER BY created_at DESC LIMIT 100")
        .bind(t).bind(id).fetch_all(&a.db).await?;
    Ok(rows.iter().map(|r| serde_json::json!({"id":r.get::<String,_>("id"), "kind":r.get::<String,_>("kind"), "number":r.get::<String,_>("number"), "locale":r.get::<String,_>("locale"), "createdAt":r.get::<String,_>("created"), "pdfPath":format!("/store-api/account/orders/{id}/receipts/{}/pdf", r.get::<String,_>("id"))})).collect())
}

/// The snapshot and stored document of one customer-visible receipt.
pub async fn receipt(
    a: &State,
    t: &str,
    order: &str,
    id: &str,
) -> Result<(Value, Option<Vec<u8>>)> {
    let r=sqlx::query("SELECT snapshot,content FROM order_receipts WHERE tenant=$1 AND order_id=$2 AND id=$3 AND customer_visible")
        .bind(t).bind(order).bind(id).fetch_optional(&a.db).await?
        .ok_or(Error(StatusCode::NOT_FOUND,"Receipt not found".into()))?;
    Ok((r.get("snapshot"), r.get("content")))
}
