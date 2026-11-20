//! Editor access to an app entity: the generic endpoint needs every right its declared actions need.
use crate::domain::{action_admission, core_extensions};
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_identity::permit;

/// The generic entity endpoint cannot bypass stricter action permissions.
pub fn entity_access(m: &Manifest, e: &Entity, h: &RequestContext, handler: &str) -> Result<()> {
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
        action_admission::additional(h, action)?;
        if let Some(scope) = action.permission.as_deref() {
            permit(h, scope)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::entity_access;
    use crate::domain::approval::built_in;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::{RequestContext, StatusCode};

    #[test]
    fn an_append_only_inbox_is_never_edited_through_the_generic_endpoint() {
        let mut m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        let entity = m.entities[0].clone();
        let mut admin = RequestContext::new();
        admin.insert("x-rac-role", "admin".parse().unwrap());
        admin.insert("x-rac-tenant", "demo".parse().unwrap());
        admin.insert("x-tenant", "demo".parse().unwrap());
        assert!(entity_access(&m, &entity, &admin, "list").is_ok());
        let mut submit = m.actions[0].clone();
        submit.handler = "submit".into();
        submit.entity = Some(entity.name.clone());
        m.actions.push(submit);
        assert_eq!(
            entity_access(&m, &entity, &admin, "save")
                .err()
                .map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
    }
}
