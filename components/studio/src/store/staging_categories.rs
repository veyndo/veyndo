//! Category release units and dependency-ordered tree publication; stock is never part of a catalog release.
use super::staging_documents::Tx;
use crate::domain::views::{CategoryUnit, Identified};
use serde_json::{Map, Value};
use veyndo_catalog::store::staging as catalog;
use veyndo_core::document::view;
use veyndo_core::{Result, bad};

/// Adds every category as a release unit, locked for the comparison.
pub async fn snapshot(tx: &mut Tx<'_>, t: &str, result: &mut Map<String, Value>) -> Result<()> {
    for v in catalog::categories(tx, t).await? {
        result.insert(
            format!("category:{}", view::<Identified>(&v).id.unwrap()),
            v,
        );
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
                let exists = catalog::category_exists(tx, t, parent).await?;
                if !exists {
                    return Err(bad("Publish the parent category with this release"));
                }
                let cycle = catalog::category_cycle(tx, t, parent, v.id.as_deref()).await?;
                if cycle {
                    return Err(bad("Category release creates a cycle"));
                }
            }
            catalog::upsert_category(
                tx,
                t,
                v.id.as_deref(),
                parent,
                v.position.unwrap_or(0) as i32,
                &v.data,
            )
            .await?;
            remaining.retain(|k| k != &key);
            progressed = true;
        }
        if !progressed {
            return Err(bad("Cyclic category release"));
        }
    }
    Ok(())
}
