//! Tenant-owned logo uploads: bounded decoding, metadata stripping, immutable PNG storage and linked public delivery.
use super::*;
use base64::Engine;
use image::ImageFormat;
use std::io::Cursor;
use veyndo_orders::store::company_logos;
fn normalize(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err(bad("Logo must contain 1 byte..2 MiB"));
    }
    let format = image::guess_format(bytes).map_err(|_| bad("Invalid company image"))?;
    if ![ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP].contains(&format) {
        return Err(bad("Logo requires PNG, JPEG or WebP"));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|_| bad("Invalid or oversized company image"))?;
    let (w, h) = (img.width(), img.height());
    let mut output = Cursor::new(Vec::new());
    img.write_to(&mut output, ImageFormat::Png)
        .map_err(|_| bad("Cannot normalize logo"))?;
    if output.get_ref().len() > 2 * 1024 * 1024 {
        return Err(bad("Normalized logo exceeds 2 MiB"));
    }
    Ok((output.into_inner(), w, h))
}
pub(super) async fn upload(
    State(a): State<App>,
    h: RequestContext,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "settings.write")?;
    let field = multipart
        .next_field()
        .await
        .map_err(|_| bad("Invalid logo upload"))?
        .ok_or(bad("Logo file required"))?;
    if field.name() != Some("file") {
        return Err(bad("Logo file required"));
    }
    let bytes = field.bytes().await.map_err(|_| bad("Logo exceeds 2 MiB"))?;
    if multipart
        .next_field()
        .await
        .map_err(|_| bad("Invalid logo upload"))?
        .is_some()
    {
        return Err(bad("Exactly one logo file required"));
    }
    let (content, w, h) = tokio::task::spawn_blocking(move || normalize(&bytes))
        .await
        .map_err(|_| bad("Cannot decode logo"))??;
    let digest = format!("{:x}", Sha256::digest(&content));
    let id = company_logos::save(&a, &t, content, &digest, w, h).await?;
    Ok(Json(
        json!({"id":id,"digest":digest,"width":w,"height":h,"published":false}),
    ))
}
async fn bytes(a: &App, t: &str, id: &str) -> Result<Response> {
    let data = company_logos::content(a, t, id).await?;
    Ok((
        [
            ("content-type", "image/png"),
            ("cache-control", "private, no-store"),
            ("x-content-type-options", "nosniff"),
        ],
        data,
    )
        .into_response())
}
pub(super) async fn preview(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "settings.read")?;
    let content = company_logos::content(&a, &t, &id).await?;
    Ok(Json(
        json!({"id":id,"dataUrl":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(content))}),
    ))
}
#[derive(Deserialize)]
pub(super) struct LogoScope {
    pub shop: String,
    #[serde(default = "default_channel")]
    pub channel: String,
}
fn default_channel() -> String {
    "default".into()
}
pub(super) async fn public(
    State(a): State<App>,
    mut h: RequestContext,
    Path(id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<LogoScope>,
) -> Result<Response> {
    if let Some(existing) = header(&h, "x-tenant")
        && existing != q.shop
    {
        return Err(bad("Logo shop scope mismatch"));
    }
    h.insert("x-tenant", q.shop.parse().map_err(|_| bad("Invalid shop"))?);
    let t = tenant(&h)?;
    company_logos::admit_public(&a, &t, &q.channel, &id).await?;
    bytes(&a, &t, &id).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_content_and_truncated_images_rejected() {
        assert!(normalize(b"<svg onload='alert(1)'/>").is_err());
        assert!(normalize(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(normalize(&vec![0; 2 * 1024 * 1024 + 1]).is_err());
    }
    #[test]
    fn normalized_png_decodes_without_metadata() {
        let img = image::DynamicImage::new_rgba8(4, 3);
        let mut b = Cursor::new(Vec::new());
        img.write_to(&mut b, ImageFormat::Png).unwrap();
        let (out, w, h) = normalize(b.get_ref()).unwrap();
        assert_eq!((w, h), (4, 3));
        assert!(image::load_from_memory(&out).is_ok());
    }
}
