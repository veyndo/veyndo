//! Editor mounts, enumerated fields and tenant-owned core references; no app alters core tables.
use super::*;
/// The generic entity endpoint cannot bypass stricter action permissions.
pub(crate) fn entity_access(
    m: &Manifest,
    e: &Entity,
    h: &RequestContext,
    handler: &str,
) -> Result<()> {
    core_extensions::access(e, h, handler == "save")?;
    if handler == "save"
        && m.actions
            .iter()
            .any(|a| a.handler == "submit" && a.entity.as_deref() == Some(e.name.as_str()))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Append-only form inbox cannot be edited or restored".into(),
        ));
    }
    for action in m
        .actions
        .iter()
        .filter(|a| a.entity.as_deref() == Some(e.name.as_str()) && a.handler == handler)
    {
        gateway::additional_permissions(h, action)?;
        if let Some(scope) = action.permission.as_deref() {
            auth::permit(h, scope)?;
        }
    }
    Ok(())
}
pub(super) use veyndo_app_platform::store::core_references::validate_references;
