//! Private unit catalog scope is derived from the current employee/cart, never caller criteria; indexed before pagination.
use crate::store::access::guard_conn;
use crate::types::model::Access;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_shop::types::cart::stored;
pub async fn scope(a: &State, h: &RequestContext) -> Result<Option<String>> {
    let Some(token) = header(h, "sw-context-token") else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT * FROM carts WHERE tenant=$1 AND token=$2")
        .bind(tenant(h)?)
        .bind(token)
        .fetch_optional(&a.db)
        .await?;
    let Some(row) = row else { return Ok(None) };
    let c = stored(&row)?;
    if c.data.b2b.is_none() {
        return Ok(None);
    };
    let policy = guard_conn(&mut *a.db.begin().await?, h, &c)
        .await?
        .ok_or(bad("Company context required"))?;
    Ok(policy
        .unit
        .restricted_catalog
        .then(|| c.data.b2b.as_ref().unwrap().unit_id.clone()))
}
pub async fn admit(a: &State, h: &RequestContext, ids: &[String]) -> Result<()> {
    let Some(unit) = scope(a, h).await? else {
        return Ok(());
    };
    let denied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products p WHERE p.tenant=$1 AND p.id=ANY($3) AND NOT EXISTS(SELECT 1 FROM b2b_catalog b WHERE b.tenant=p.tenant AND b.unit_id=$2 AND (b.product_id=p.id OR b.product_id=p.parent_id OR EXISTS(SELECT 1 FROM products child WHERE child.tenant=p.tenant AND child.parent_id=p.id AND child.id=b.product_id))))").bind(tenant(h)?).bind(unit).bind(ids).fetch_one(&a.db).await?;
    if denied {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Product unavailable in company catalog".into(),
        ));
    }
    Ok(())
}
pub async fn save(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = veyndo_identity::merchant(h)?;
    veyndo_identity::permit(h, "b2b.manage")?;
    veyndo_identity::permit(h, "catalog.read")?;
    let mut tx = a.db.begin().await?;
    veyndo_core::history_context::context(&mut tx, h, "merchant").await?;
    let result = save_tx(&mut tx, &t, v).await?;
    tx.commit().await?;
    Ok(result)
}
/// The JSON fields `save_tx` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveTxInput {
    #[serde(deserialize_with = "lenient")]
    unit_id: Option<String>,
    product_ids: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Same catalog admission and revision fence for direct edits and atomic releases.
pub async fn save_tx(conn: &mut sqlx::PgConnection, t: &str, v: &Value) -> Result<Value> {
    let input: SaveTxInput = view(v);
    let unit = input.unit_id.as_deref().ok_or(bad("Unit required"))?;
    let ids: Vec<String> = serde_json::from_value(input.product_ids.clone())
        .map_err(|_| bad("Product ID list required"))?;
    if ids.len() > 1000 || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len() {
        return Err(bad("Catalog edit accepts up to 1000 distinct product IDs"));
    }
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM b2b_units WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(t)
            .bind(unit)
            .fetch_optional(&mut *conn)
            .await?;
    let revision = revision.ok_or(Error(StatusCode::NOT_FOUND, "Unit not found".into()))?;
    if input.revision != Some(revision) {
        return Err(conflict("Unit catalog changed; reload before saving"));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM products WHERE tenant=$1 AND id=ANY($2)")
            .bind(t)
            .bind(&ids)
            .fetch_one(&mut *conn)
            .await?;
    if count != ids.len() as i64 {
        return Err(bad("Catalog references missing or foreign products"));
    }
    sqlx::query("DELETE FROM b2b_catalog WHERE tenant=$1 AND unit_id=$2")
        .bind(t)
        .bind(unit)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO b2b_catalog(tenant,unit_id,product_id) SELECT $1,$2,unnest($3::text[])",
    )
    .bind(t)
    .bind(unit)
    .bind(ids)
    .execute(&mut *conn)
    .await?;
    sqlx::query("UPDATE b2b_units SET revision=revision+1 WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(unit)
        .execute(&mut *conn)
        .await?;
    crate::store::organization::event(
        conn,
        t,
        "b2b.catalog_changed",
        json!({"unitId":unit,"revision":revision+1}),
    )
    .await?;
    Ok(json!({"revision":revision+1}))
}

pub async fn admit_unit(
    conn: &mut sqlx::PgConnection,
    t: &str,
    unit: &str,
    product: &str,
    policy: &Access,
) -> Result<()> {
    if !policy.unit.restricted_catalog {
        return Ok(());
    }
    let admitted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products p JOIN b2b_catalog b ON b.tenant=p.tenant AND (b.product_id=p.id OR b.product_id=p.parent_id) WHERE p.tenant=$1 AND p.id=$2 AND b.unit_id=$3)").bind(t).bind(product).bind(unit).fetch_one(conn).await?;
    if !admitted {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Product not found in company catalog".into(),
        ));
    }
    Ok(())
}
