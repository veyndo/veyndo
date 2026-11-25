//! Rules, promotions, flows and sales channels of a sandbox shop: copying, locked snapshot reads and release upserts, inside the caller's transaction.
use serde_json::Value;
use veyndo_core::{Result, bad};

/// The release units of one kind (`rule`, `promotion`, `flow`, `channel`) by ID, locked.
pub async fn units(
    c: &mut sqlx::PgConnection,
    t: &str,
    kind: &str,
) -> Result<Vec<(String, Value)>> {
    Ok(match kind {
        "rule" => sqlx::query!(r#"SELECT id,jsonb_build_object('name',name,'condition',condition,'active',active) AS "data!" FROM commerce_rules WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut *c).await?.into_iter().map(|r| (r.id, r.data)).collect(),
        "promotion" => sqlx::query!("SELECT id,data FROM commerce_promotions WHERE tenant=$1 ORDER BY id FOR UPDATE", t).fetch_all(&mut *c).await?.into_iter().map(|r| (r.id, r.data)).collect(),
        "flow" => sqlx::query!("SELECT id,data FROM commerce_flows WHERE tenant=$1 ORDER BY id FOR UPDATE", t).fetch_all(&mut *c).await?.into_iter().map(|r| (r.id, r.data)).collect(),
        _ => sqlx::query!("SELECT id,data FROM sales_channels WHERE tenant=$1 ORDER BY id FOR UPDATE", t).fetch_all(&mut *c).await?.into_iter().map(|r| (r.id, r.data)).collect(),
    })
}

/// Inserts or replaces a rule.
pub async fn upsert_rule(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    name: &Value,
    condition: &Value,
    active: Option<bool>,
) -> Result<()> {
    sqlx::query!("INSERT INTO commerce_rules(tenant,id,name,condition,active) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET name=EXCLUDED.name,condition=EXCLUDED.condition,active=EXCLUDED.active,revision=commerce_rules.revision+1", t, id, name, condition, active).execute(&mut *c).await?;
    Ok(())
}

/// Inserts or replaces a promotion, flow or channel document (`table` is one of their fixed tables).
pub async fn upsert_unit(
    c: &mut sqlx::PgConnection,
    table: &str,
    t: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    match table {
        "commerce_promotions" => sqlx::query!(
            "INSERT INTO commerce_promotions(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_promotions.revision+1",
            t,
            id,
            data
        )
        .execute(&mut *c)
        .await?,
        "commerce_flows" => sqlx::query!(
            "INSERT INTO commerce_flows(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_flows.revision+1",
            t,
            id,
            data
        )
        .execute(&mut *c)
        .await?,
        "sales_channels" => sqlx::query!(
            "INSERT INTO sales_channels(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=sales_channels.revision+1",
            t,
            id,
            data
        )
        .execute(&mut *c)
        .await?,
        _ => return Err(bad("Unsupported release unit")),
    };
    Ok(())
}

/// A flow document.
pub async fn flow(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_flows WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_one(&mut *c)
    .await?)
}

/// Copies promotions, flows and sales channels into the sandbox.
pub async fn copy_promotions_flows_channels(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_promotions(tenant,id,data) SELECT $2,id,data FROM commerce_promotions WHERE tenant=$1",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    sqlx::query!(
        "INSERT INTO commerce_flows(tenant,id,data) SELECT $2,id,data FROM commerce_flows WHERE tenant=$1",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    sqlx::query!(
        "INSERT INTO sales_channels(tenant,id,data) SELECT $2,id,data FROM sales_channels WHERE tenant=$1",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Copies the rules into the sandbox.
pub async fn copy_rules(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!("INSERT INTO commerce_rules(tenant,id,name,condition,active) SELECT $2,id,name,condition,active FROM commerce_rules WHERE tenant=$1", t, id).execute(&mut *c).await?;
    Ok(())
}
