//! Immediate App Studio preview: the native runtime in an expiring personal environment. No build, version or release is created.
use crate::domain::app_generation::default_actions;
use crate::domain::build_admission;
use crate::domain::consent::actor_permissions;
use crate::ports::Platform;
use crate::service::app_builds::live;
use crate::store::package_install::install_preview;
use crate::store::records::reset_preview;
use crate::store::{environments, packages};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::view;
use veyndo_core::store::lease::Lease;
use veyndo_core::{RequestContext, Result, bad, process_config};
use veyndo_identity::permit;

/// Whether a package can run without review: native only, with no schedule, event or payment.
pub fn immediate(m: &Manifest) -> bool {
    m.runtime == "declarative"
        && m.schedules.is_empty()
        && m.events.is_empty()
        && m.payment_provider.is_none()
}

/// Installs the draft into the personal preview environment of the caller, creating that environment on first use.
/// The installed package an install reports, passed on as reported.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Installed {
    id: Value,
    version: Value,
    digest: Value,
}

/// Previews a package in the merchant's personal environment; the request's `manifest` is the app's open manifest document.
pub async fn run(a: &Platform, h: &RequestContext, mut v: Value) -> Result<Value> {
    let t = live(a, h).await?;
    let actor = h
        .principal
        .user
        .as_deref()
        .filter(|v| *v != "bootstrap")
        .ok_or(bad("Personal preview needs a personal session"))?;
    if v["manifest"].to_string().len() > 65536 {
        return Err(bad("Preview app exceeds 64 KiB"));
    }
    default_actions(&mut v["manifest"], &[])?;
    let m: Manifest =
        serde_json::from_value(v["manifest"].clone()).map_err(|e| bad(e.to_string()))?;
    build_admission::validate(&m, &process_config::get().publishers)?;
    if !immediate(&m) {
        return Err(bad(
            "Immediate preview runs native apps only; external services, schedules and payments require reviewed staging",
        ));
    }
    actor_permissions(h, &m)?;
    let _lease = Lease::acquire(a, &t, &format!("app-preview:{actor}"), 1).await?;
    let existing = environments::preview_of(a, &t, actor).await?;
    let environment = if let Some(id) = existing {
        id
    } else {
        permit(h, "settings.write")?;
        a.actions.create_preview(h, actor).await?
    };
    let mut tx = a.db.begin().await?;
    a.commerce.lock(&mut tx, &environment).await?;
    environments::lock_preview(&mut tx, &environment, &t, actor).await?;
    let count = packages::count_other(&mut tx, &environment, &m.id).await?;
    if count >= 20 {
        return Err(bad(
            "Personal preview supports at most 20 apps; reuse the current app ID",
        ));
    }
    // A changed data model starts with empty preview records; view-only hot reload preserves them.
    let previous = packages::manifest_value(&mut tx, &environment, &m.id).await?;
    if let Some(value) = previous {
        let old: Manifest =
            serde_json::from_value(value).map_err(|_| bad("Invalid previous preview"))?;
        if old.entities != m.entities {
            reset_preview(&mut tx, &environment, &m.id).await?;
        }
    }
    let result = install_preview(a.rules, a.commerce, &mut tx, &environment, m).await?;
    environments::extend_preview(&mut tx, &environment).await?;
    tx.commit().await?;
    let installed: Installed = view(&result);
    Ok(
        json!({"environment":environment,"app":installed.id,"version":installed.version,"digest":installed.digest,"private":true,"expiresInSeconds":3600,"releaseAllowed":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::immediate;
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn only_native_apps_without_schedules_events_or_payments_run_immediately() {
        let native: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/product-advisor/manifest.json"
        ))
        .unwrap();
        let mut plain = native.clone();
        plain.runtime = "declarative".into();
        plain.schedules.clear();
        plain.events.clear();
        plain.payment_provider = None;
        assert!(immediate(&plain));
        let mut service = plain.clone();
        service.runtime = "service".into();
        assert!(!immediate(&service));
        let mut listening = plain.clone();
        listening.events = serde_json::from_value(json!(["order.placed"])).unwrap();
        assert!(!immediate(&listening));
    }
}
