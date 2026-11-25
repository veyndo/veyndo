//! Facts beside the intelligence component's own data: products (existence, intake identity, images, texts, stock), customers, carts, sandboxes, scenario runs, knowledge sources, hypotheses and legal acknowledgements. Columns checked at compile time where the row is not handed to another component.
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// Whether the entity a native view names exists in the shop; None for a view without entity references.
pub async fn entity_exists(
    e: impl PgExecutor<'_>,
    view: &str,
    tenant: &str,
    id: &str,
) -> Result<Option<bool>> {
    Ok(Some(match view {
        "productData" => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
                tenant,
                id
            )
            .fetch_one(e)
            .await?
        }
        "orders" => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND id=$2) AS "exists!""#,
                tenant,
                id
            )
            .fetch_one(e)
            .await?
        }
        "customers" => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM customers WHERE tenant=$1 AND id=$2) AS "exists!""#,
                tenant,
                id
            )
            .fetch_one(e)
            .await?
        }
        _ => return Ok(None),
    }))
}

/// The `extra` document of a product, if the product exists.
pub async fn extra(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT extra FROM products WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(e)
    .await?)
}

/// The bytes of an owned PNG, JPEG or WebP product image of at most 8 MiB.
pub async fn owned_image(
    e: impl PgExecutor<'_>,
    tenant: &str,
    id: &str,
) -> Result<Option<Vec<u8>>> {
    Ok(sqlx::query_scalar!("SELECT content FROM product_assets WHERE tenant=$1 AND id=$2 AND mime IN ('image/png','image/jpeg','image/webp') AND octet_length(content)<=8388608",
        tenant, id).fetch_optional(e).await?)
}

/// The name and description of a product.
pub struct ProductText {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// The texts of the named products of a shop.
pub async fn texts(
    e: impl PgExecutor<'_>,
    tenant: &str,
    ids: &[String],
) -> std::result::Result<Vec<ProductText>, sqlx::Error> {
    sqlx::query_as!(
        ProductText,
        "SELECT id,name,description FROM products WHERE tenant=$1 AND id=ANY($2)",
        tenant,
        ids
    )
    .fetch_all(e)
    .await
}

/// Moves a hypothesis to `state` at the expected revision; false when the revision changed.
pub async fn decide_hypothesis(
    e: impl PgExecutor<'_>,
    tenant: &str,
    id: &str,
    state: &str,
    revision: i64,
) -> Result<bool> {
    Ok(sqlx::query!("UPDATE knowledge_hypotheses SET state=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4",
        state, tenant, id, revision).execute(e).await?.rows_affected() == 1)
}

/// Records the cart's legal acknowledgement, replacing an earlier one.
pub async fn accept_legal(
    e: impl PgExecutor<'_>,
    tenant: &str,
    cart: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO legal_acceptances(tenant,cart_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,cart_id) DO UPDATE SET data=EXCLUDED.data",
        tenant, cart, data).execute(e).await?;
    Ok(())
}

/// A product as the catalog maps it, with its parent.
pub async fn product(
    a: &veyndo_core::State,
    tenant: &str,
    id: &str,
) -> Result<Option<(veyndo_pricing::catalog::Product, Option<String>)>> {
    use sqlx::Row;
    // runtime SQL: the whole row goes to veyndo_catalog::store::products::product, which maps a PgRow.
    let row = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?;
    Ok(row.map(|r| {
        (
            veyndo_catalog::store::products::product(&r),
            r.get("parent_id"),
        )
    }))
}

/// A product's name and stock.
pub struct ProductStock {
    pub name: String,
    pub stock: i32,
}

/// The name and stock of a product.
pub async fn stock(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<Option<ProductStock>> {
    Ok(sqlx::query_as!(
        ProductStock,
        "SELECT name,stock FROM products WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(e)
    .await?)
}

/// The ID of the shop customer with this email.
pub async fn customer_by_email(
    e: impl PgExecutor<'_>,
    tenant: &str,
    email: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM customers WHERE tenant=$1 AND email=$2",
        tenant,
        email
    )
    .fetch_optional(e)
    .await?)
}

/// The stored cart document and status.
pub struct CartState {
    pub data: Value,
    pub status: String,
}

/// The stored cart, row-locked.
pub async fn cart_locked(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<CartState> {
    Ok(sqlx::query_as!(
        CartState,
        "SELECT data,status FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(e)
    .await?)
}

/// Whether the shop is a private sandbox (an environment without a preview owner).
pub async fn private_sandbox(e: impl PgExecutor<'_>, tenant: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NULL) AS "private!""#,
        tenant
    )
    .fetch_one(e)
    .await?)
}

/// One scenario run as the scenario list reads it.
pub struct ScenarioRun {
    pub id: String,
    pub run_request: Value,
    pub run_scopes: Value,
    pub updated_text: String,
}

/// The actor's latest scenario runs under this agent identity, of one group or all (at most 120).
pub async fn scenario_runs(
    e: impl PgExecutor<'_>,
    tenant: &str,
    actor: &str,
    group: Option<&str>,
    agent: Option<&str>,
) -> Result<Vec<ScenarioRun>> {
    Ok(sqlx::query_as!(ScenarioRun, r#"SELECT id,run_request AS "run_request!",run_scopes AS "run_scopes!",run_updated::text AS "updated_text!" FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'scenario' AND (run_request->>'agentId') IS NOT DISTINCT FROM $4 AND ($3::text IS NULL OR run_request->'scenario'->>'group'=$3) ORDER BY run_updated DESC,id LIMIT 120"#,
        tenant, actor, group, agent).fetch_all(e).await?)
}

/// The identity of a knowledge source.
pub struct SourceRef {
    pub content_hash: String,
    pub product_id: Option<String>,
    pub locale: String,
}

/// An unarchived knowledge source.
pub async fn source(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<Option<SourceRef>> {
    Ok(sqlx::query_as!(SourceRef, "SELECT content_hash,product_id,locale FROM knowledge_documents WHERE tenant=$1 AND id=$2 AND NOT archived",
        tenant, id).fetch_optional(e).await?)
}

/// The first 16 chunk texts of a source in one language.
pub async fn source_chunks(
    e: impl PgExecutor<'_>,
    tenant: &str,
    id: &str,
    locale: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT text FROM knowledge_chunks WHERE tenant=$1 AND document_id=$2 AND locale=$3 ORDER BY position LIMIT 16",
        tenant, id, locale).fetch_all(e).await?)
}
