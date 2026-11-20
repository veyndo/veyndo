//! Typed API and bounded upload write source hashes, chunks and graph relations atomically.
use super::*;
use axum::extract::Multipart;
use veyndo_intelligence::store::documents as store;
pub(crate) async fn ingest(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "knowledge.write")?;
    Ok(Json(save(&a, &h, &tenant(&h)?, &v, "api").await?))
}
pub(crate) async fn upload(
    State(a): State<App>,
    h: RequestContext,
    mut form: Multipart,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.write")?;
    let mut v = json!({});
    let mut file = None;
    let mut names = std::collections::HashSet::new();
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|_| bad("Invalid multipart document"))?
    {
        let name = field.name().unwrap_or("").to_owned();
        if !names.insert(name.clone()) {
            return Err(bad("Duplicate upload field"));
        }
        if name == "file" {
            let pdf = field
                .file_name()
                .is_some_and(|n| n.to_lowercase().ends_with(".pdf"));
            let bytes = field
                .bytes()
                .await
                .map_err(|_| bad("Invalid document upload"))?;
            file = Some((bytes.to_vec(), pdf));
        } else if name == "translations" {
            let text = field
                .text()
                .await
                .map_err(|_| bad("Invalid translations"))?;
            if text.len() > 200_000 {
                return Err(bad("Translations too long"));
            }
            v[name] = serde_json::from_str(&text).map_err(|_| bad("Invalid translation JSON"))?;
        } else if ["title", "productId", "kind", "locale"].contains(&name.as_str()) {
            let text = field
                .text()
                .await
                .map_err(|_| bad("Invalid document metadata"))?;
            if text.len() > 200 {
                return Err(bad("Metadata too long"));
            }
            v[name] = json!(text);
        } else {
            return Err(bad("Unsupported upload field"));
        }
    }
    let (bytes, pdf) = file.ok_or(bad("File required"))?;
    v["content"] = json!(parser::text(bytes, pdf).await?);
    Ok(Json(
        save(&a, &h, &t, &v, if pdf { "pdf" } else { "upload-text" }).await?,
    ))
}
async fn save(a: &App, h: &RequestContext, t: &str, v: &Value, source: &str) -> Result<Value> {
    guidance::admit(h, &v["kind"])?;
    let data = content::validate(a, t, v).await?;
    store::save(a, h, t, &data, source).await
}
pub(crate) async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.read")?;
    Ok(Json(store::list(&a, &t).await?))
}
pub(crate) async fn publish(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.write")?;
    Ok(Json(store::publish(&a, &h, &t, &id, &v).await?))
}
