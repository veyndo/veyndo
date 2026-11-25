//! Clone and publish purchasing configuration only; employees, orders, approvals and spending remain live-only.
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
pub async fn clone_configuration(
    conn: &mut sqlx::PgConnection,
    live: &str,
    stage: &str,
) -> Result<()> {
    sqlx::query!("INSERT INTO b2b_companies(tenant,id,data,revision) SELECT $2,id,data,revision FROM b2b_companies WHERE tenant=$1", live, stage).execute(&mut *conn).await?;
    sqlx::query!("INSERT INTO b2b_units(tenant,id,company_id,parent_id,data,revision) SELECT $2,id,company_id,parent_id,data,revision FROM b2b_units WHERE tenant=$1", live, stage).execute(&mut *conn).await?;
    sqlx::query!("INSERT INTO b2b_catalog(tenant,unit_id,product_id) SELECT $2,unit_id,product_id FROM b2b_catalog WHERE tenant=$1", live, stage).execute(conn).await?;
    Ok(())
}
pub async fn snapshot_configuration(
    conn: &mut sqlx::PgConnection,
    t: &str,
    result: &mut serde_json::Map<String, Value>,
) -> Result<()> {
    for r in sqlx::query!(
        "SELECT id,data FROM b2b_companies WHERE tenant=$1 ORDER BY id FOR UPDATE",
        t
    )
    .fetch_all(&mut *conn)
    .await?
    {
        result.insert(format!("b2b-company:{}", r.id.clone()), r.data.clone());
    }
    for r in sqlx::query!(
        "SELECT id,company_id,parent_id,data FROM b2b_units WHERE tenant=$1 ORDER BY id FOR UPDATE",
        t
    )
    .fetch_all(&mut *conn)
    .await?
    {
        let id: String = r.id.clone();
        let catalog: Vec<String> = sqlx::query_scalar!(
            "SELECT product_id FROM b2b_catalog WHERE tenant=$1 AND unit_id=$2 ORDER BY product_id",
            t,
            &id
        )
        .fetch_all(&mut *conn)
        .await?;
        result.insert(format!("b2b-unit:{id}"),json!({"companyId":r.company_id.clone(),"parentId":r.parent_id.clone(),"data":r.data.clone(),"catalogIds":catalog}));
    }
    Ok(())
}
/// Call after jointly selected products/settings have been written, in the same release transaction.
pub async fn publish_configuration(
    conn: &mut sqlx::PgConnection,
    t: &str,
    keys: &[String],
    current: &Value,
) -> Result<()> {
    for key in keys.iter().filter(|k| k.starts_with("b2b-company:")) {
        let id = &key[12..];
        let revision: Option<i64> = sqlx::query_scalar!(
            "SELECT revision FROM b2b_companies WHERE tenant=$1 AND id=$2",
            t,
            id
        )
        .fetch_optional(&mut *conn)
        .await?;
        crate::store::organization::save_tx(
            conn,
            t,
            "company.save",
            &json!({"id":id,"revision":revision.unwrap_or(0),"data":current[key]}),
        )
        .await?;
    }
    let mut pending = keys
        .iter()
        .filter(|k| k.starts_with("b2b-unit:"))
        .collect::<Vec<_>>();
    while !pending.is_empty() {
        let index = pending
            .iter()
            .position(|key| {
                let parent = current[key.as_str()]["parentId"].as_str();
                !parent.is_some_and(|p| {
                    pending
                        .iter()
                        .any(|k| k.as_str() == format!("b2b-unit:{p}"))
                })
            })
            .ok_or(bad("Selected purchasing units contain a hierarchy cycle"))?;
        let key = pending.remove(index);
        let id = &key[9..];
        let value = &current[key];
        let revision: Option<i64> = sqlx::query_scalar!(
            "SELECT revision FROM b2b_units WHERE tenant=$1 AND id=$2",
            t,
            id
        )
        .fetch_optional(&mut *conn)
        .await?;
        let saved=crate::store::organization::save_tx(conn,t,"unit.save",&json!({"id":id,"revision":revision.unwrap_or(0),"companyId":value["companyId"],"parentId":value["parentId"],"data":value["data"]})).await?;
        crate::store::catalog::save_tx(
            conn,
            t,
            &json!({"unitId":id,"revision":saved["revision"],"productIds":value["catalogIds"]}),
        )
        .await?;
    }
    Ok(())
}
