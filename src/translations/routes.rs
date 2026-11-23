//! Authorized translation job creation, progress, paginated drafts and resume/cancel controls.
use super::*;
pub(crate) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    auth::permit(&h, "catalog")?;
    let t = merchant(&a, &h)?;
    let (settings, _) = commerce::config(&a, &t).await?;
    let choice: Choice = serde_json::from_value(
        v.get("inference")
            .cloned()
            .unwrap_or(json!({"provider":"ollama","model":null})),
    )
    .map_err(|_| bad("Invalid translation provider"))?;
    let products = selection::parse(v.get("productIds"))?;
    admit_provider(&a, &choice).await?;
    let mut tx = a.db.begin().await?;
    let id = Uuid::new_v4().to_string();
    enqueue(
        &a,
        &mut tx,
        &t,
        &id,
        &settings,
        (
            v["targetLocale"]
                .as_str()
                .ok_or(bad("Target language required"))?,
            &choice,
            v["overwrite"].as_bool().unwrap_or(false),
            products.as_deref(),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"status":"queued"})))
}
/// Both explicit settings commands and approved merchant tasks enqueue the same durable draft owner.
pub(crate) async fn enqueue(
    _a: &App,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    s: &commerce::Settings,
    request: (&str, &Choice, bool, Option<&[String]>),
) -> Result<()> {
    let (target, choice, overwrite, products) = request;
    veyndo_catalog::http::translation_jobs::enqueue(
        tx,
        t,
        id,
        (&s.main_locale, &s.locales),
        (target, &json!(choice), overwrite, products),
    )
    .await
}
pub(crate) use veyndo_catalog::http::translation_jobs::{control, detail, list};
/// Resolve centrally managed provider settings before acquiring a transaction-pool backend.
pub(crate) async fn admit_provider(a: &App, choice: &Choice) -> Result<()> {
    let available = a.inference.public_providers().await.map_err(bad)?;
    if !available["providers"].as_array().is_some_and(|providers| {
        providers
            .iter()
            .any(|p| p["id"] == json!(choice.provider) && p["configured"] == true)
    }) {
        return Err(bad("Translation provider is not configured"));
    }
    Ok(())
}
