//! Aggregate control-plane reads across live shops (sandboxes excluded): totals, amounts per currency, shop pages and daily order timelines. Products and accounts come from the catalog and identity stores, the rest through the dependants port.
use crate::ports::dependants;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// Platform totals for a window of `days`, with the time they were read.
pub struct Totals {
    pub shops: i64,
    pub sandboxes: i64,
    pub users: i64,
    pub products: i64,
    pub customers: i64,
    pub orders: i64,
    pub pending_events: i64,
    pub generated: String,
}

/// The platform totals inside the caller's read snapshot.
pub async fn totals(conn: &mut PgConnection, days: i64) -> Result<Totals> {
    let shops = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM tenants t WHERE NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id)) AS "shops!",array(SELECT tenant FROM shop_environments) AS "sandboxes!",now()::text AS "generated!""#
    )
    .fetch_one(&mut *conn)
    .await?;
    let sandboxes = shops.sandboxes;
    Ok(Totals {
        shops: shops.shops,
        sandboxes: sandboxes.len() as i64,
        users: veyndo_identity::store::shop_reads::user_count(&mut *conn).await?,
        products: veyndo_catalog::store::shop_reads::count_outside(&mut *conn, &sandboxes).await?,
        customers: dependants()?
            .customers_outside(&mut *conn, &sandboxes)
            .await?,
        orders: dependants()?
            .orders_outside(&mut *conn, days, &sandboxes)
            .await?,
        pending_events: veyndo_core::store::outbox::pending::outside(&mut *conn, &sandboxes)
            .await?,
        generated: shops.generated,
    })
}

/// The tenants of all sandboxes, on the caller's connection.
async fn sandboxes(conn: &mut PgConnection) -> Result<Vec<String>> {
    Ok(
        sqlx::query_scalar!(r#"SELECT tenant AS "tenant!" FROM shop_environments"#)
            .fetch_all(conn)
            .await?,
    )
}

/// Recorded, simulated and confirmed captured amounts of one currency, as decimal text.
pub struct Amounts {
    pub currency: String,
    pub orders: i64,
    pub booked: String,
    pub simulated: String,
    pub captured: String,
}

/// The amounts per currency of all live shops, or of `tenant`, for a window of `days`.
pub async fn amounts(
    conn: &mut PgConnection,
    days: i64,
    tenant: Option<&str>,
) -> Result<Vec<Amounts>> {
    let excluded = sandboxes(&mut *conn).await?;
    dependants()?.amounts(conn, days, tenant, &excluded).await
}

/// One live shop of the operator list with its counts.
pub struct ShopRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub status_revision: i64,
    pub created: String,
    pub products: i64,
    pub customers: i64,
    pub members: i64,
    pub orders: i64,
    pub apps: i64,
    pub channels: i64,
    pub sources: i64,
}

/// Up to `limit` live shops after the cursor whose name or ID contains `search`.
pub async fn shops(
    a: &State,
    after: &str,
    search: &str,
    limit: i64,
    days: i64,
) -> Result<Vec<ShopRow>> {
    let shops = sqlx::query!(
        r#"SELECT t.id,t.name,t.status,t.status_revision,t.created_at::text AS "created!",(SELECT count(*) FROM sales_channels WHERE tenant=t.id AND coalesce((data->>'active')::boolean,true)) AS "channels!" FROM tenants t WHERE t.id>$1 AND (strpos(lower(t.name),lower($2))>0 OR strpos(t.id,lower($2))>0) AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id) ORDER BY t.id LIMIT $3"#,
        after,
        search,
        limit
    )
    .fetch_all(&a.db)
    .await?;
    let ids: Vec<String> = shops.iter().map(|s| s.id.clone()).collect();
    let products: std::collections::HashMap<String, i64> =
        veyndo_catalog::store::shop_reads::count_by_shop(a, &ids)
            .await?
            .into_iter()
            .collect();
    let members: std::collections::HashMap<String, i64> =
        veyndo_identity::store::shop_reads::members_by_shop(a, &ids)
            .await?
            .into_iter()
            .collect();
    let activity = dependants()?.activity(&ids, days).await?;
    Ok(shops
        .into_iter()
        .map(|s| {
            let held = activity.get(&s.id).copied().unwrap_or_default();
            ShopRow {
                products: products.get(&s.id).copied().unwrap_or(0),
                members: members.get(&s.id).copied().unwrap_or(0),
                customers: held.customers,
                orders: held.orders,
                apps: held.apps,
                sources: held.sources,
                channels: s.channels,
                id: s.id,
                name: s.name,
                status: s.status,
                status_revision: s.status_revision,
                created: s.created,
            }
        })
        .collect())
}

/// The name of a live shop; none for an unknown shop or a sandbox.
pub async fn live_name(a: &State, id: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT name FROM tenants t WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id)",
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The order count of one day.
pub struct Day {
    pub day: String,
    pub orders: i64,
}

/// The orders of a shop per day for the last `days` days, oldest first.
pub async fn timeline(id: &str, days: i64) -> Result<Vec<Day>> {
    dependants()?.timeline(id, days).await
}
