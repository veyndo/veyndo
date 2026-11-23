//! Product attachments and paid digital downloads: bounded binary persistence and tenant/account ACL.
use crate::*;
mod import;
pub(crate) use import::image as import_image;
mod caption_source;
mod captions;
pub(crate) mod design_references;
mod image_jobs;
pub(crate) use captions::schema as caption_schema;
mod rich;
mod rich_document;
pub(crate) use rich_document::safe_url;
pub(crate) mod ingestion;
mod model_format;
mod upload;
pub(crate) use upload::persist as persist_app_upload;
mod app_files;
pub(crate) use app_files::callback as app_file_callback;
pub(crate) use image_jobs::image_once;
pub(crate) use rich::validate_rich;
pub(crate) use upload::{list, publish};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/merchant/media/caption",
            &[("POST", "assistant.use")],
            post(captions::route),
        )
        .secure_route(
            "/api/merchant/media/provider",
            &[("GET", "catalog.read")],
            get(image_jobs::provider),
        )
        .secure_route(
            "/api/merchant/products/{id}/media/jobs",
            &[("GET", "catalog.read"), ("POST", "catalog.write")],
            get(image_jobs::list_jobs).post(image_jobs::enqueue),
        )
        .secure_route(
            "/api/merchant/media/jobs/{id}",
            &[("GET", "catalog.read")],
            get(image_jobs::detail),
        )
        .secure_route(
            "/api/merchant/media/jobs/{id}/apply",
            &[("POST", "catalog.write")],
            post(image_jobs::apply),
        )
        .secure_route(
            "/api/merchant/products/{id}/assets",
            &[("GET", "catalog.read"), ("POST", "catalog.write")],
            get(veyndo_catalog::http::assets::list).post(veyndo_catalog::http::assets::upload),
        )
        .secure_route(
            "/api/merchant/assets/{id}",
            &[("GET", "catalog.read"), ("PUT", "catalog.write")],
            get(veyndo_catalog::http::downloads::preview)
                .put(veyndo_catalog::http::assets::publish),
        )
        .route(
            "/store-api/product/{id}/attachments",
            get(veyndo_catalog::http::downloads::attachments),
        )
        .route(
            "/store-api/assets/{id}",
            get(veyndo_catalog::http::downloads::attachment),
        )
        .route(
            "/store-api/account/downloads",
            get(veyndo_orders::http::downloads::owned),
        )
        .route(
            "/store-api/orders/{order}/downloads/{asset}",
            get(veyndo_orders::http::downloads::file),
        )
        .layer(axum::extract::DefaultBodyLimit::max(
            model_format::MODEL_LIMIT + 65536,
        ))
}
pub(crate) fn media_permission(name: &str) -> Option<&'static str> {
    match name {
        "merchant.media.caption" => Some("assistant.use"),
        "merchant.media.provider" | "merchant.media.list" | "merchant.media.detail" => {
            Some("catalog.read")
        }
        "merchant.media.create" | "merchant.media.apply" => Some("catalog.write"),
        _ => None,
    }
}
pub(crate) async fn media_invoke(
    a: &App,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let state = State(a.clone());
    let headers = h.clone();
    let id = || {
        v["id"]
            .as_str()
            .map(String::from)
            .ok_or(bad("Image ID required"))
    };
    let Json(result) = match name {
        "merchant.media.caption" => return captions::prepare(a, h, v).await,
        "merchant.media.provider" => image_jobs::provider(state, headers).await?,
        "merchant.media.create" => {
            image_jobs::enqueue(
                state,
                headers,
                Path(
                    v["productId"]
                        .as_str()
                        .ok_or(bad("Product ID required"))?
                        .into(),
                ),
                Json(v.clone()),
            )
            .await?
        }
        "merchant.media.list" => {
            image_jobs::list_jobs(
                state,
                headers,
                Path(
                    v["productId"]
                        .as_str()
                        .ok_or(bad("Product ID required"))?
                        .into(),
                ),
            )
            .await?
        }
        "merchant.media.detail" => image_jobs::detail(state, headers, Path(id()?)).await?,
        "merchant.media.apply" => {
            image_jobs::apply(state, headers, Path(id()?), Json(v.clone())).await?
        }
        _ => return Err(bad("Unknown media capability")),
    };
    Ok(result)
}
