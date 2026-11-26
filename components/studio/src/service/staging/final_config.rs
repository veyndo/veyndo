//! Validate the final published channel/automation aggregate and initialize native POS defaults.
use super::Baseline;
use super::snapshot::Tx;
use crate::ports::ReleaseHost;
use crate::store::staging_release as db;
use serde_json::Value;
use veyndo_core::{RequestContext, Result, bad};
use veyndo_marketing::store::channel_config::validate_channel_config;
use veyndo_marketing::store::configuration_lifecycle::validate_references;
use veyndo_pos::store::channel_setup::configure as configure_pos;
use veyndo_shop::domain::defaults::decode;
/// Validates the final published channel and automation aggregate and initializes native POS defaults.
pub async fn validate<H: ReleaseHost>(
    host: &H,
    tx: &mut Tx<'_>,
    t: &str,
    h: &RequestContext,
    keys: &[String],
    (current, actual): (&Value, &Value),
    base: &mut Baseline,
) -> Result<()> {
    if keys.iter().any(|key| key == "settings") {
        let settings = decode(db::settings(tx, t).await?)?;
        host.settings_guards()
            .locale(tx, t, &settings.main_locale)
            .await?;
    }
    // Validate references against the final aggregate, including jointly selected new rules.
    for key in keys {
        if let Some((kind, unit)) = key.split_once(':') {
            if ["rule", "promotion", "flow", "channel"].contains(&kind) {
                validate_references(
                    tx,
                    t,
                    if kind == "rule" { "rules" } else { kind },
                    unit,
                    &current[key],
                )
                .await?;
            }
            if kind == "flow" {
                host.validate_flow(tx, t, h, &current[key]).await?;
            }
            if kind == "channel"
                && !matches!(
                    current[key]["visibility"].as_str(),
                    None | Some("public" | "private")
                )
            {
                return Err(bad("Invalid channel visibility"));
            }
            if kind == "channel" && unit == "default" && current[key]["kind"] != "storefront" {
                return Err(bad("Main channel must remain storefront"));
            }
            if kind == "channel" {
                let settings = decode(db::settings(tx, t).await?)?;
                validate_channel_config(tx, t, unit, &current[key], &settings).await?;
                let configured = db::channel_configured(tx, t, unit).await?;
                configure_pos(
                    tx,
                    t,
                    unit,
                    &current[key],
                    !actual.as_object().unwrap().contains_key(key) && !configured,
                )
                .await?;
                if current[key]["kind"] == "pos" && !configured {
                    let patch = db::channel_overrides(tx, t, unit).await?;
                    base.record(&format!("settings-channel:{unit}"), None, Some(patch))?;
                }
            }
        }
    }
    Ok(())
}
