//! Category release units and dependency-ordered tree publication; stock is never part of a catalog release.
use super::staging_documents::Tx;
use crate::domain::views::{CategoryUnit, Identified};
use serde_json::{Map, Value};
use veyndo_core::document::view;
use veyndo_core::{Result, bad};

/// Adds every category as a release unit, locked for the comparison.
pub async fn snapshot(tx: &mut Tx<'_>, t: &str, result: &mut Map<String, Value>) -> Result<()> {
    for v in sqlx::query_scalar!(r#"SELECT to_jsonb(c)-'tenant'-'revision' AS "data!" FROM categories c WHERE tenant=$1 ORDER BY id FOR UPDATE"#, t).fetch_all(&mut **tx).await? {
        result.insert(format!("category:{}",view::<Identified>(&v).id.unwrap()),v);
    }
    Ok(())
}
/// Publishes the selected categories parents first; a missing parent or a cycle is refused.
pub async fn publish(tx: &mut Tx<'_>, t: &str, current: &Value, keys: &[String]) -> Result<()> {
    let mut remaining = keys
        .iter()
        .filter(|k| k.starts_with("category:"))
        .cloned()
        .collect::<Vec<_>>();
    while !remaining.is_empty() {
        let mut progressed = false;
        for key in remaining.clone() {
            let v: CategoryUnit = view(&current[&key]);
            let parent = v.parent_id.as_deref();
            if let Some(parent) = parent {
                if remaining.contains(&format!("category:{parent}")) {
                    continue;
                }
                let exists: bool = sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant=$1 AND id=$2) AS "exists!""#, t, parent)
                .fetch_one(&mut **tx)
                .await?;
                if !exists {
                    return Err(bad("Publish the parent category with this release"));
                }
                let cycle:bool=sqlx::query_scalar!(r#"WITH RECURSIVE parents AS (SELECT id,parent_id FROM categories WHERE tenant=$1 AND id=$2 UNION ALL SELECT c.id,c.parent_id FROM categories c JOIN parents p ON c.id=p.parent_id WHERE c.tenant=$1) SELECT EXISTS(SELECT 1 FROM parents WHERE id=$3) AS "exists!""#, t, parent, v.id.as_deref()).fetch_one(&mut **tx).await?;
                if cycle {
                    return Err(bad("Category release creates a cycle"));
                }
            }
            sqlx::query!("INSERT INTO categories(tenant,id,parent_id,position,data) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET parent_id=EXCLUDED.parent_id,position=EXCLUDED.position,data=EXCLUDED.data,revision=categories.revision+1", t, v.id.as_deref(), parent, v.position.unwrap_or(0) as i32, &v.data).execute(&mut **tx).await?;
            remaining.retain(|k| k != &key);
            progressed = true;
        }
        if !progressed {
            return Err(bad("Cyclic category release"));
        }
    }
    Ok(())
}
