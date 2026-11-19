//! Entry points of category writes for capabilities, imports and history; the rules and storage are owned by the catalog component.
use super::*;
use axum::extract::FromRef;
use veyndo_catalog::ports::Catalog;
use veyndo_catalog::service::categories as service;
pub(crate) use veyndo_catalog::types::category::input_schema;

/// Review uses the exact native DTO/data/language checks before creating executable work.
pub(crate) async fn admit_configuration(
    a: &App,
    t: &str,
    value: &Value,
    create: bool,
) -> Result<()> {
    service::admit(&Catalog::from_ref(a), t, value, create).await
}
pub(crate) async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    veyndo_catalog::http::categories::list(State(Catalog::from_ref(&a)), h).await
}
pub(crate) async fn import(a: App, h: RequestContext, id: String, v: Value) -> Result<Json<Value>> {
    Ok(Json(
        service::write(&Catalog::from_ref(&a), &h, &id, v, true).await?,
    ))
}
pub(crate) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    let id = match v.get("id") {
        None | Some(Value::Null) => Uuid::new_v4().to_string(),
        Some(Value::String(id)) if apps::identifier(id) || Uuid::parse_str(id).is_ok() => {
            id.clone()
        }
        _ => return Err(bad("Invalid category ID")),
    };
    v.as_object_mut()
        .ok_or(bad("Invalid category edit"))?
        .remove("id");
    import(a, h, id, v).await
}
pub(crate) async fn save(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    veyndo_catalog::http::categories::save(State(Catalog::from_ref(&a)), h, Path(id), Json(v)).await
}
