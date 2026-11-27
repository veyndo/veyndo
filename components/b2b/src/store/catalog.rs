//! Private unit catalog scope is derived from the current employee/cart, never caller criteria; indexed before pagination. The unit's listed product ids are read here and checked against product identity from the records port in the same transaction.
use crate::ports::{CatalogProduct, records};
use crate::store::access::guard_conn;
use crate::types::events::CatalogChanged;
use crate::types::model::Access;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
pub async fn scope(a: &State, h: &RequestContext) -> Result<Option<String>> {
    let Some(token) = header(h, "sw-context-token") else {
        return Ok(None);
    };
    let Some(c) = super::carts::by_token(a, &tenant(h)?, token).await? else {
        return Ok(None);
    };
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
    let t = tenant(h)?;
    let mut tx = a.db.begin().await?;
    let listed = listed(&mut tx, &t, &unit).await?;
    let denied = records()?
        .catalog_products(&mut tx, &t, ids, &listed)
        .await?
        .iter()
        .any(|p| !listed_directly(p, &listed) && !p.variant_listed);
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
    veyndo_core::store::history_context::context(&mut tx, h, "merchant").await?;
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
    let revision: Option<i64> = sqlx::query_scalar!(
        "SELECT revision FROM b2b_units WHERE tenant=$1 AND id=$2 FOR UPDATE",
        t,
        unit
    )
    .fetch_optional(&mut *conn)
    .await?;
    let revision = revision.ok_or(Error(StatusCode::NOT_FOUND, "Unit not found".into()))?;
    if input.revision != Some(revision) {
        return Err(conflict("Unit catalog changed; reload before saving"));
    }
    let count = records()?.products_found(conn, t, &ids).await?;
    if count != ids.len() as i64 {
        return Err(bad("Catalog references missing or foreign products"));
    }
    sqlx::query!(
        "DELETE FROM b2b_catalog WHERE tenant=$1 AND unit_id=$2",
        t,
        unit
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "INSERT INTO b2b_catalog(tenant,unit_id,product_id) SELECT $1,$2,unnest($3::text[])",
        t,
        unit,
        &ids
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE b2b_units SET revision=revision+1 WHERE tenant=$1 AND id=$2",
        t,
        unit
    )
    .execute(&mut *conn)
    .await?;
    let changed = CatalogChanged {
        unit_id: unit,
        revision: revision + 1,
    };
    crate::store::organization::event(conn, t, &changed).await?;
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
    let listed = listed(conn, t, unit).await?;
    let admitted = records()?
        .catalog_products(conn, t, &[product.to_string()], &listed)
        .await?
        .iter()
        .any(|p| listed_directly(p, &listed));
    if !admitted {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Product not found in company catalog".into(),
        ));
    }
    Ok(())
}

/// The product ids listed in the private catalogue of unit `unit`.
pub async fn listed(conn: &mut sqlx::PgConnection, t: &str, unit: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT product_id FROM b2b_catalog WHERE tenant=$1 AND unit_id=$2",
        t,
        unit
    )
    .fetch_all(conn)
    .await?)
}

/// Whether the product or its parent is listed.
pub fn listed_directly(p: &CatalogProduct, listed: &[String]) -> bool {
    listed.contains(&p.id)
        || p.parent_id
            .as_ref()
            .is_some_and(|parent| listed.contains(parent))
}
