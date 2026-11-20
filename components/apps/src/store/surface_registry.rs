//! Current-authority app surface projection shared by storefronts and native visual composition.
use crate::domain::action_admission::authorized;
use crate::domain::action_policy::readonly;
use crate::domain::mcp_views::{available, location};
use crate::domain::surfaces::surface_url;
use crate::domain::{approval, native_views};
use crate::store::packages::active_manifests;
use serde_json::{Value, json};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{RequestContext, Result, State, process_config};
use veyndo_identity::{merchant, permit};

/// The main language of a shop and the languages it has enabled.
pub struct Languages<'a> {
    pub main: &'a str,
    pub enabled: &'a Value,
}

/// The surfaces a caller may see, with the actions it may run and the languages of the shop.
pub async fn registry(
    a: &State,
    h: &RequestContext,
    public: bool,
    languages: Languages<'_>,
) -> Result<Value> {
    if !public {
        merchant(h)?;
    }
    let services = &process_config::get().services;
    let mut surfaces = vec![];
    let mut public_apps = vec![];
    for m in active_manifests(a, h).await? {
        if m.actions.iter().any(|act| act.public) {
            public_apps.push(m.id.clone());
        }
        for s in &m.surfaces {
            if s.location.starts_with("admin.") == public
                || s.permission
                    .as_deref()
                    .is_some_and(|p| permit(h, p).is_err())
            {
                continue;
            }
            if location(&s.location) && !available(h, &m, s) {
                continue;
            }
            if let Some(mut view) = native_views::payload(&m, s) {
                let allowed = s
                    .actions
                    .iter()
                    .filter(|name| {
                        m.actions
                            .iter()
                            .any(|act| act.name == **name && authorized(h, act))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(blocks) = view["view"]["blocks"].as_array_mut() {
                    blocks.retain(|b| {
                        ["text", "button", "frame", "tabs"]
                            .iter()
                            .any(|kind| b["kind"] == *kind)
                            || b["readAction"]
                                .as_str()
                                .is_some_and(|n| allowed.iter().any(|a| a == n))
                    });
                    for b in blocks {
                        if b["kind"] == "form"
                            && b["writeAction"]
                                .as_str()
                                .is_none_or(|n| !allowed.iter().any(|a| a == n))
                        {
                            b["kind"] = json!("cards");
                            b.as_object_mut().unwrap().remove("writeAction");
                        }
                    }
                }
                view["tenant"] = json!(tenant(h)?);
                let mut surface = json!(s);
                surface["actions"] = json!(allowed);
                surfaces.push(json!({"app":m.id,"version":m.version,"surface":surface,"native":view,"readOnlyActions":s.actions.iter().filter(|name|m.actions.iter().any(|act|act.name==**name && authorized(h,act) && readonly(act))).collect::<Vec<_>>(),"mainLocale":languages.main,"locales":languages.enabled}));
            } else if approval::approved(&m, &services[&m.id])
                && let Some(url) = surface_url(&m.id, &s.ui_path, services)
            {
                surfaces.push(json!({"app":m.id,"version":m.version,"surface":s,"url":url,"mainLocale":languages.main,"locales":languages.enabled,"readOnlyActions":s.actions.iter().filter(|name|m.actions.iter().any(|act|act.name==**name && authorized(h,act) && readonly(act))).collect::<Vec<_>>()}));
            }
        }
    }
    Ok(json!({"surfaces":surfaces,"publicApps":public_apps,"mainLocale":languages.main}))
}
