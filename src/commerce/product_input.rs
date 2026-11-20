//! Admit native product edits and expand explicit category-only changes through the same revisioned editor.
use super::*;
pub(crate) use veyndo_catalog::domain::product_admission::{
    admit_product_write, product_write_guidance,
};
use veyndo_catalog::domain::product_admission::{category_assignment, category_edit};
pub(super) async fn expand_category_edit(
    a: &App,
    h: &RequestContext,
    id: &str,
    v: Value,
) -> Result<Value> {
    if v.get("categoryIds").is_none() {
        return Ok(v);
    }
    auth::permit(h, "catalog")?;
    let edit = category_edit(&v)?;
    let current =
        super::product_edit::product_editor(State(a.clone()), h.clone(), Path(id.to_owned()))
            .await?
            .0;
    category_assignment(&edit, &current)
}
