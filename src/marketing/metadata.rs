//! Authorized revision-bound source facts for products, customers and orders; secrets and pricing authority are excluded.
use super::*;
pub(crate) use veyndo_marketing::domain::metadata::validate as validate_metadata;
pub(crate) fn router() -> Router<App> {
    Router::new().secure_route(
        "/api/automation/entities/{entity}/{id}",
        &[("PUT", "automation.manage")],
        axum::routing::put(save),
    )
}
async fn save(
    State(a): State<App>,
    h: RequestContext,
    Path((entity, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(
        &h,
        match entity.as_str() {
            "products" => "catalog.write",
            "customers" => "customers.write",
            "orders" => "orders.write",
            _ => return Err(bad("Unknown rule entity")),
        },
    )?;
    validate_metadata(&entity, &v["data"])?;
    let rev = v["revision"].as_i64().ok_or(bad("Revision required"))?;
    veyndo_marketing::store::rule_entities::save(&a, &t, &entity, &id, &v["data"], rev).await?;
    Ok(Json(json!({"saved":true,"revision":rev+1})))
}
