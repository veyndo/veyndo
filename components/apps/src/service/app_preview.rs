//! Immediate App Studio preview: the native runtime in an expiring personal environment. No build, version or release is created.
use crate::domain::app_generation::default_actions;
use crate::domain::build_admission;
use crate::domain::consent::actor_permissions;
use crate::ports::Platform;
use crate::service::app_builds::live;
use crate::store::package_install::install_preview;
use crate::store::records::reset_preview;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::database::lease::Lease;
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
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT tenant FROM shop_environments WHERE live_tenant=$1 AND preview_owner=$2",
    )
    .bind(&t)
    .bind(actor)
    .fetch_optional(&a.db)
    .await?;
    let environment = if let Some(id) = existing {
        id
    } else {
        permit(h, "settings.write")?;
        a.actions.create_preview(h, actor).await?
    };
    let mut tx = a.db.begin().await?;
    a.commerce.lock(&mut tx, &environment).await?;
    sqlx::query("SELECT tenant FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 AND preview_owner=$3 FOR UPDATE").bind(&environment).bind(&t).bind(actor).fetch_one(&mut *tx).await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM app_packages WHERE tenant=$1 AND id<>$2")
            .bind(&environment)
            .bind(&m.id)
            .fetch_one(&mut *tx)
            .await?;
    if count >= 20 {
        return Err(bad(
            "Personal preview supports at most 20 apps; reuse the current app ID",
        ));
    }
    // A changed data model starts with empty preview records; view-only hot reload preserves them.
    let previous: Option<Value> =
        sqlx::query_scalar("SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2")
            .bind(&environment)
            .bind(&m.id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(value) = previous {
        let old: Manifest =
            serde_json::from_value(value).map_err(|_| bad("Invalid previous preview"))?;
        if old.entities != m.entities {
            reset_preview(&mut tx, &environment, &m.id).await?;
        }
    }
    let result = install_preview(a.rules, a.commerce, &mut tx, &environment, m).await?;
    sqlx::query(
        "UPDATE shop_environments SET preview_until=now()+interval '1 hour' WHERE tenant=$1",
    )
    .bind(&environment)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(
        json!({"environment":environment,"app":result["id"],"version":result["version"],"digest":result["digest"],"private":true,"expiresInSeconds":3600,"releaseAllowed":false}),
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
