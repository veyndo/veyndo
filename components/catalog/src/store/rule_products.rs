//! Product rows as rules, flows and channels read and change them: complete products for rule facts, a customer's review count, the flow's download grants of an order, the revision-bound automation write and the channel's visibility count. Marketing calls these directly on its own connection. Columns checked at compile time.
use crate::store::products::ProductRow;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;

/// The products among `ids`, in no particular order.
pub async fn by_ids(conn: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<Vec<Product>> {
    Ok(sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND id=ANY($2)"#, tenant, ids)
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(Product::from)
        .collect())
}

/// How many reviews customer `customer` wrote.
pub async fn customer_reviews(
    conn: &mut PgConnection,
    tenant: &str,
    customer: Option<&str>,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "reviews!" FROM product_reviews WHERE tenant=$1 AND session='customer:'||$2"#, tenant, customer)
        .fetch_one(conn)
        .await?)
}

/// Removes every download grant of order `order`.
pub async fn revoke_downloads(conn: &mut PgConnection, tenant: &str, order: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM order_downloads WHERE tenant=$1 AND order_id=$2",
        tenant,
        order
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Grants order `order` the download assets of the products its cart lines reference.
pub async fn grant_downloads(
    conn: &mut PgConnection,
    tenant: &str,
    order: &str,
    document: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO order_downloads(tenant,order_id,asset_id) SELECT $1,$2,a.id FROM product_assets a WHERE a.tenant=$1 AND a.kind='download' AND EXISTS(SELECT 1 FROM jsonb_array_elements($3::jsonb->'cart'->'lineItems') l WHERE l->>'referencedId'=a.product_id) ON CONFLICT DO NOTHING", tenant, order, document).execute(conn).await?;
    Ok(())
}

/// Stores the automation data of product `id` only at revision `rev`; the number of rows changed.
pub async fn save_automation_at(
    a: &State,
    tenant: &str,
    id: &str,
    data: &Value,
    rev: i64,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE products SET extra=jsonb_set(extra,'{automation}',$1),revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4", data, tenant, id, rev).execute(&a.db).await?.rows_affected())
}

/// How many product visibility rows channel `channel` has.
pub async fn visibility_count(conn: &mut PgConnection, tenant: &str, channel: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM product_channel_visibility WHERE tenant=$1 AND channel_id=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// An admitted product and its parent.
pub struct Admitted {
    pub id: String,
    pub parent_id: Option<String>,
}

/// Those of `ids` that are active, under an active parent and not hidden in `channel`.
pub async fn visible(
    a: &State,
    tenant: &str,
    channel: &str,
    ids: &[String],
) -> Result<Vec<Admitted>> {
    Ok(sqlx::query_as!(Admitted, "SELECT p.id,p.parent_id FROM products p WHERE p.tenant=$1 AND p.id=ANY($3) AND p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$2 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id))",
        tenant, channel, ids)
    .fetch_all(&a.db)
    .await?)
}

/// The ids among `ids` that `visible` admits.
pub async fn visible_ids(
    a: &State,
    tenant: &str,
    ids: &[String],
    channel: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT p.id FROM products p WHERE p.tenant=$1 AND p.id=ANY($2) AND p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$3 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id))",
        tenant, ids, channel)
    .fetch_all(&a.db)
    .await?)
}
