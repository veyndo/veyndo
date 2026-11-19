//! Revision-bound category writes, bounded translations and serialized cycle-safe tree moves.
use crate::ports::Catalog;
use crate::store::categories;
use crate::types::category::{Category, Edit};
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::{merchant, permit};

async fn configured(c: &Catalog, tenant: &str, edit: &Edit) -> Result<()> {
    let languages = c.shop.languages(tenant).await?;
    edit.validate_for(languages.as_ref())
}

/// Review uses the exact native DTO, data and language checks before creating executable work.
pub async fn admit(c: &Catalog, tenant: &str, value: &Value, create: bool) -> Result<()> {
    let mut value = value.clone();
    if create {
        value
            .as_object_mut()
            .ok_or(bad("Invalid category edit"))?
            .remove("id");
    }
    configured(c, tenant, &Edit::parse(value)?).await
}

/// The merchant tree with the shop's main language; refuses trees beyond the interactive limit.
pub async fn list(c: &Catalog, h: &RequestContext) -> Result<(String, Vec<Category>)> {
    permit(h, "catalog.read")?;
    let tenant = merchant(h)?;
    let tree = categories::tree(&c.core, &tenant).await?;
    if tree.len() > 2000 {
        return Err(bad("Category tree exceeds interactive limit"));
    }
    Ok((c.shop.languages(&tenant).await?.main, tree))
}

/// Creates or updates one category and returns its new revision.
pub async fn write(
    c: &Catalog,
    h: &RequestContext,
    id: &str,
    v: Value,
    create: bool,
) -> Result<Value> {
    permit(h, "catalog")?;
    let tenant = merchant(h)?;
    let edit = Edit::parse(v)?;
    configured(c, &tenant, &edit).await?;
    let mut tx = c.core.db.begin().await?;
    c.shop.attribute(&mut tx, h, "merchant").await?;
    categories::lock_tree(&mut tx, &tenant).await?;
    if let Some(parent) = &edit.parent_id
        && !categories::parent_admissible(&mut tx, &tenant, parent, id).await?
    {
        return Err(bad("Category parent is unavailable or creates a cycle"));
    }
    let revision = if create {
        categories::insert(&mut tx, &tenant, id, &edit).await?;
        1
    } else {
        let revision = edit.revision.ok_or(bad("Category revision required"))?;
        if !categories::update(&mut tx, &tenant, id, &edit, revision).await? {
            return Err(conflict("Category changed"));
        }
        revision + 1
    };
    tx.commit().await?;
    Ok(json!({"id":id,"revision":revision,"saved":true}))
}
