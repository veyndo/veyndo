//! Company logo transport: one multipart upload, a merchant preview and the public delivery of linked logos.
use crate::domain::logo_image::normalize;
use crate::store::company_logos;
use axum::Json;
use axum::extract::{Multipart, Path, Query, State as Shared};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};

/// Stores one uploaded logo after normalization; it is published only when the company basis links it.
pub async fn upload(
    Shared(a): Shared<State>,
    h: RequestContext,
    mut multipart: Multipart,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "settings.write")?;
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

/// The stored logo as a data URL for the merchant's own preview.
pub async fn preview(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "settings.read")?;
    let content = company_logos::content(&a, &t, &id).await?;
    Ok(Json(
        json!({"id":id,"dataUrl":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(content))}),
    ))
}

/// The shop and channel a public logo request names.
#[derive(Deserialize)]
pub struct LogoScope {
    pub shop: String,
    #[serde(default = "default_channel")]
    pub channel: String,
}
fn default_channel() -> String {
    "default".into()
}

/// Serves a logo the shop's or an active channel's company basis links.
pub async fn public(
    Shared(a): Shared<State>,
    mut h: RequestContext,
    Path(id): Path<String>,
    Query(q): Query<LogoScope>,
) -> Result<Response> {
    if let Some(existing) = header(&h, "x-tenant")
        && existing != q.shop
    {
        return Err(bad("Logo shop scope mismatch"));
    }
    h.insert("x-tenant", q.shop.parse().map_err(|_| bad("Invalid shop"))?);
    let t = tenant(&h)?;
    company_logos::admit_public(&a, &t, &q.channel, &id).await?;
    let data = company_logos::content(&a, &t, &id).await?;
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
