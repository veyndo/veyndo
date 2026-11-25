//! The tenant lock of marketing configuration, the revision of one definition and the admission of rule references.
use serde_json::Value;
use sqlx::{PgConnection, Postgres, Transaction};
use veyndo_core::{Error, Result, StatusCode, bad};

/// Serializes every change to the marketing configuration of a shop.
pub async fn lock(tx: &mut Transaction<'_, Postgres>, t: &str) -> Result<()> {
    super::configuration_writes::advisory_lock(tx, &format!("{t}:automation")).await
}
/// The current revision of one definition, optionally locked for a change.
pub async fn revision(
    conn: &mut PgConnection,
    t: &str,
    kind: &str,
    id: &str,
    locked: bool,
) -> Result<i64> {
    super::configuration_writes::revision(conn, t, kind, id, locked)
        .await?
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "Configuration not found".into(),
        ))
}
/// Every referenced rule must exist in this shop, without cycles and within bounded depth.
pub async fn validate_references(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    kind: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    fn collect(v: &Value, ids: &mut std::collections::HashSet<String>) {
        match v {
            Value::Object(o) => {
                if o.get("type").and_then(Value::as_str) == Some("ruleReference")
                    && let Some(id) = o.get("ruleId").and_then(Value::as_str)
                {
                    ids.insert(id.into());
                }
                for v in o.values() {
                    collect(v, ids);
                }
            }
            Value::Array(a) => {
                for v in a {
                    collect(v, ids);
                }
            }
            _ => (),
        }
    }
    let mut pending = std::collections::HashSet::new();
    collect(data, &mut pending);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..=8 {
        let ids = pending
            .drain()
            .filter(|id| !seen.contains(id))
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(());
        }
        if seen.len() + ids.len() > 100 || (kind == "rules" && ids.iter().any(|i| i == id)) {
            return Err(bad("Cyclic or excessive rule references"));
        }
        let rows = sqlx::query!(
            "SELECT id,condition FROM commerce_rules WHERE tenant=$1 AND id=ANY($2)",
            t,
            &ids
        )
        .fetch_all(&mut **tx)
        .await?;
        if rows.len() != ids.len() {
            return Err(bad("Referenced rule not found in this shop"));
        }
        seen.extend(ids);
        for row in rows {
            collect(&row.condition, &mut pending);
        }
    }
    Err(bad("Rule reference depth exceeded"))
}
