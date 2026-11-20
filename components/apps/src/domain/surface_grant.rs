//! Short-lived UI grants bind a package, surface, actor and concrete editor object; these are the rules without storage.
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};

/// The caller a grant is bound to: a digest of its merchant, cart and customer credentials.
pub fn actor(h: &RequestContext) -> String {
    hash(&format!(
        "{}:{}:{}",
        header(h, "authorization").unwrap_or(""),
        header(h, "sw-context-token").unwrap_or(""),
        header(h, "x-customer-token").unwrap_or("")
    ))
}
/// Keeps an action on the object its surface was opened for.
pub fn bind_input(m: &Manifest, s: &Surface, ctx: &Value, name: &str, v: &Value) -> Result<()> {
    for (key, id) in ctx
        .as_object()
        .ok_or(bad("Invalid stored surface context"))?
    {
        if v.get(key).is_some_and(|supplied| supplied != id) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Action targets a different surface object".into(),
            ));
        }
    }
    for block in m
        .views
        .iter()
        .filter(|view| s.ui_path == format!("native/{}", view.id))
        .flat_map(|v| &v.blocks)
    {
        if let Some(binding) = &block.context_binding {
            let value = ctx
                .get(&binding.key)
                .ok_or(bad("Editor surface needs a context object"))?;
            if block.write_action.as_deref() == Some(name) && v["fields"][&binding.field] != *value
            {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Record must remain bound to the surface object".into(),
                ));
            }
            if block.read_action.as_deref() == Some(name) && v["filter"][&binding.field] != *value {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Read must filter by the surface object".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{actor, bind_input};
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::{RequestContext, StatusCode};

    fn manifest() -> Manifest {
        serde_json::from_str(include_str!(
            "../../../../extensions/apps/assistant-examples/mcp-app.json"
        ))
        .unwrap()
    }

    #[test]
    fn a_grant_subject_changes_with_any_credential() {
        let anonymous = RequestContext::new();
        let mut shopper = RequestContext::new();
        shopper.insert("sw-context-token", "cart-1".parse().unwrap());
        let mut other = RequestContext::new();
        other.insert("sw-context-token", "cart-2".parse().unwrap());
        assert_eq!(actor(&shopper), actor(&shopper));
        assert_ne!(actor(&shopper), actor(&other));
        assert_ne!(actor(&shopper), actor(&anonymous));
        assert_eq!(actor(&anonymous).len(), 64);
    }

    #[test]
    fn an_editor_surface_keeps_reads_and_writes_on_its_object() {
        let m = manifest();
        let s = &m.surfaces[0];
        let ctx = json!({"productId":"mug"});
        let save = json!({"id":"a","fields":{"product_id":"mug","title":"Care"}});
        assert!(bind_input(&m, s, &ctx, "save_entries", &save).is_ok());
        let foreign = json!({"id":"a","fields":{"product_id":"vase"}});
        assert_eq!(
            bind_input(&m, s, &ctx, "save_entries", &foreign)
                .err()
                .map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        assert!(
            bind_input(
                &m,
                s,
                &ctx,
                "list_entries",
                &json!({"filter":{"product_id":"mug"}})
            )
            .is_ok()
        );
        assert!(bind_input(&m, s, &ctx, "list_entries", &json!({})).is_err());
        // A supplied context key must repeat the stored object.
        assert!(
            bind_input(
                &m,
                s,
                &ctx,
                "list_entries",
                &json!({"productId":"vase","filter":{"product_id":"mug"}})
            )
            .is_err()
        );
        // Without a stored object the editor surface cannot run at all.
        assert!(bind_input(&m, s, &json!({}), "save_entries", &save).is_err());
        assert!(bind_input(&m, s, &json!("mug"), "save_entries", &save).is_err());
        // A surface without bound blocks only checks repeated context keys.
        assert!(bind_input(&m, &m.surfaces[1], &json!({}), "list_entries", &json!({})).is_ok());
    }
}
