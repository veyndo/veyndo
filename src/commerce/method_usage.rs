//! Authoritative deletion guards for methods; the reference counts are owned by the shipping component.
use super::*;
use veyndo_shipping::store::method_usage::{Area, usage};
pub(crate) async fn method_dependencies(
    State(a): State<App>,
    h: RequestContext,
    path: Path<(String, String)>,
) -> Result<Json<Value>> {
    let Json(found) =
        veyndo_shipping::http::method_usage::dependencies(State(a.core.clone()), h, path).await?;
    Ok(Json(json!(found)))
}
pub(super) async fn guard(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    old: &Settings,
    next: &Settings,
    channel: Option<&str>,
) -> Result<()> {
    payments::capture_configuration::validate(tx, t, next).await?;
    for (area, previous, current) in [
        (
            Area::Shipping,
            old.shipping
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            next.shipping
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            Area::Payments,
            old.payments
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            next.payments
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
        ),
    ] {
        for id in previous.into_iter().filter(|id| !current.contains(id)) {
            if !usage(tx, t, area, id, channel).await?.can_delete {
                return Err(conflict("Method is referenced; deactivate it instead"));
            }
        }
    }
    Ok(())
}
