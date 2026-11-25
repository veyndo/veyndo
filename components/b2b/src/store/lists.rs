//! Saved purchase lists and requisitions of a company unit, serialized per list by an advisory lock (checked columns), and the product reads that resolve or admit their lines through the records port.
use crate::ports::records;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// A list the buyer may see.
pub struct VisibleList {
    pub id: String,
    pub data: Value,
    pub revision: i64,
    pub owner_id: String,
}

/// The unit's lists the buyer owns or that are shared, by id (at most 100).
pub async fn visible(
    conn: &mut PgConnection,
    tenant: &str,
    unit: &str,
    buyer: &str,
) -> Result<Vec<VisibleList>> {
    Ok(sqlx::query_as!(VisibleList, "SELECT id,data,revision,owner_id FROM b2b_lists WHERE tenant=$1 AND unit_id=$2 AND (owner_id=$3 OR data->>'shared'='true') ORDER BY id LIMIT 100",
        tenant, unit, buyer)
    .fetch_all(conn)
    .await?)
}

/// Serializes every change to list `id` of the shop.
pub async fn serialize(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        format!("b2b:list:{tenant}:{id}")
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// A stored list, locked for a change; `shared` says whether its document marks it shared.
pub struct LockedList {
    pub unit_id: String,
    pub owner_id: String,
    pub data: Value,
    pub shared: bool,
    pub revision: i64,
}

/// List `id`, locked for this transaction.
pub async fn locked(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<LockedList>> {
    Ok(sqlx::query_as!(LockedList, r#"SELECT unit_id,owner_id,data,coalesce(data->'shared'='true'::jsonb,false) AS "shared!",revision FROM b2b_lists WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        tenant, id)
    .fetch_optional(conn)
    .await?)
}

/// Removes list `id`.
pub async fn delete(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM b2b_lists WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// How many of `ids` are products of the shop.
pub async fn products_found(conn: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<i64> {
    records()?.products_found(conn, tenant, ids).await
}

/// Who a list belongs to.
pub struct Owner<'a> {
    pub unit: &'a str,
    pub owner: &'a str,
}

/// Inserts list `id`, or replaces its document and advances its revision.
pub async fn upsert(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    owner: Owner<'_>,
    data: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO b2b_lists(tenant,id,unit_id,owner_id,data) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET data=excluded.data,revision=b2b_lists.revision+1",
        tenant, id, owner.unit, owner.owner, data)
    .execute(conn)
    .await?;
    Ok(())
}

/// Inserts a new list `id` at its first revision.
pub async fn insert(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    owner: Owner<'_>,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO b2b_lists(tenant,id,unit_id,owner_id,data)VALUES($1,$2,$3,$4,$5)",
        tenant,
        id,
        owner.unit,
        owner.owner,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// A product with its product number.
pub struct Sku {
    pub id: String,
    pub product_number: String,
}

/// The products whose numbers are among `skus`.
pub async fn by_sku(conn: &mut PgConnection, tenant: &str, skus: &[String]) -> Result<Vec<Sku>> {
    records()?.products_by_sku(conn, tenant, skus).await
}
