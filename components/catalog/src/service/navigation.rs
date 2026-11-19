//! Public navigation is localized and restricted to the selected channel's active category ancestry.
use crate::domain::navigation::entries;
use crate::ports::{Catalog, Visit};
use crate::store::categories;
use crate::types::category::Category;
use serde_json::{Value, json};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};

async fn tree(c: &Catalog, visit: &Visit) -> Result<Vec<Category>> {
    let tree = categories::active_tree(&c.core, &visit.tenant, &visit.navigation_root).await?;
    if tree.len() > 2000 {
        return Err(bad("Category navigation exceeds limit"));
    }
    Ok(tree)
}

/// Refuses a category that is not part of the navigation of the visitor's sales channel.
pub async fn admit(c: &Catalog, h: &RequestContext, id: &str) -> Result<()> {
    let visit = c.shop.visit(h).await?;
    if !tree(c, &visit)
        .await?
        .iter()
        .any(|category| category.id == id)
    {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Category unavailable in this sales channel".into(),
        ));
    }
    Ok(())
}

pub async fn list(c: &Catalog, h: &RequestContext) -> Result<Value> {
    let visit = c.shop.visit(h).await?;
    let main = c.shop.languages(&visit.tenant).await?.main;
    let tree = tree(c, &visit).await?;
    Ok(json!({"elements":entries(&tree, &visit.locale, &main),"locale":visit.locale}))
}
