//! Rebuild a product edit from an audited snapshot; stock stays current and all associations/media pass normal admission.
use super::*;
pub(super) async fn restore(
    a: App,
    h: RequestContext,
    id: String,
    state: Value,
    revision: i64,
) -> Result<Json<Value>> {
    let Json(mut current) =
        commerce::product_editor(State(a.core.clone()), h.clone(), Path(id.clone())).await?;
    if current["revision"] != revision {
        return Err(conflict("Product changed; reload before restoring"));
    }
    let t = merchant(&a, &h)?;
    let (settings, _) = commerce::config(&a, &t).await?;
    veyndo_studio::domain::product_restore::edit(
        &mut current,
        &state,
        &id,
        &settings.locales,
        &settings.main_locale,
    );
    commerce::edit_product(State(a.core.clone()), h, Path(id), Json(current)).await
}
