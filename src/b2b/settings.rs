//! B2B feature switches live in shared commerce settings; disabling never relaxes existing financial policy.
use super::*;
use veyndo_shop::types::b2b::Config;
/// A typed feature edit delegates to the existing revisioned settings writer, audit and outbox.
pub(crate) async fn save(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    auth::permit(h, "settings.write")?;
    let t = merchant(a, h)?;
    let (settings, _) = commerce::config(a, &t).await?;
    let mut data = json!(settings);
    let config: Config = serde_json::from_value(v["data"].clone())
        .map_err(|_| bad("Invalid B2B feature configuration"))?;
    data["b2b"] = json!(config);
    Ok(commerce::save_config(
        State(axum::extract::FromRef::from_ref(a)),
        h.clone(),
        Json(json!({"revision":v["revision"],"data":data})),
    )
    .await?
    .0)
}
