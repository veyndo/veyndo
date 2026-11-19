//! Public attachments honor sales-channel visibility; a file is always delivered as a private download.
use crate::domain::asset_file::deliverable_for;
use crate::domain::content::translated_string;
use crate::ports::Catalog;
use crate::store::{assets, products};
use crate::types::asset::StoredFile;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::shop_scope::{header as request_header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_identity::{merchant, permit};

/// Delivers a file as an attachment that is neither cached nor sniffed.
pub fn response(file: StoredFile) -> Response {
    (
        [
            (header::CONTENT_TYPE, file.mime),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file.filename),
            ),
            (header::CACHE_CONTROL, "private, no-store".into()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        file.content,
    )
        .into_response()
}

pub async fn attachments(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    c.shop.admit_product(&h, &id).await?;
    let tenant = tenant(&h)?;
    let main = c.shop.languages(&tenant).await?.main;
    let locale = request_header(&h, "x-commerce-locale").unwrap_or(&main);
    let elements = assets::public_attachments(&c.core, &tenant, &id)
        .await?
        .into_iter()
        .map(|a| {
            let name = translated_string(&a.title, locale, &main).to_owned();
            json!({"id":a.id,"title":a.title,"name":name,"filename":a.filename,"mime":a.mime,"bytes":a.bytes})
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({"elements":elements})))
}

/// The product in whose context a file is requested; a variant names itself to load its parent's 3D model.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelContext {
    product_id: Option<String>,
}

pub async fn attachment(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
    Query(context): Query<ModelContext>,
) -> Result<Response> {
    let tenant = tenant(&h)?;
    let missing = || Error(StatusCode::NOT_FOUND, "Attachment not found".into());
    let file = assets::public_attachment(&c.core, &tenant, &id)
        .await?
        .ok_or_else(missing)?;
    let selected = context.product_id.as_deref().unwrap_or(&file.product_id);
    if selected != file.product_id {
        let parent = products::parent_of(&c.core, &tenant, selected).await?;
        if !deliverable_for(&file.product_id, selected, parent.as_deref(), &file.mime) {
            return Err(missing());
        }
    }
    c.shop.admit_product(&h, selected).await?;
    Ok(response(file))
}

/// Authenticated Studio preview never requires activating or publishing an unfinished product.
pub async fn preview(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Response> {
    let tenant = merchant(&h)?;
    permit(&h, "catalog.read")?;
    let file = assets::model(&a, &tenant, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Model unavailable".into()))?;
    Ok(response(file))
}
