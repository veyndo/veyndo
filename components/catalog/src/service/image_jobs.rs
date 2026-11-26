//! Durable image jobs: tenant admission, revision-bound private previews and explicit publication, without automatic paid retries.
use crate::domain::image_job::{self, Mode};
use crate::ports::Catalog;
use crate::store::media_jobs;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, uid};
use veyndo_identity::{merchant, permit};

pub async fn provider(c: &Catalog, h: &RequestContext) -> Result<Value> {
    merchant(h)?;
    permit(h, "catalog.read")?;
    let provider = c.images.provider().await?;
    Ok(json!({"configured":provider.configured,"model":provider.model}))
}

/// The product revision an image request or decision was made at, and the source image it names.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImageDecision {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    source_id: Value,
}

/// A queued image job as it was stored.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StoredImageJob {
    #[serde(deserialize_with = "lenient")]
    mode: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    prompt: Option<String>,
}

/// Queues one image job for a product at the revision the merchant saw.
pub async fn enqueue(c: &Catalog, h: &RequestContext, product: &str, v: &Value) -> Result<Value> {
    permit(h, "catalog")?;
    let t = merchant(h)?;
    let request = image_job::parse(v)?;
    let input: ImageDecision = view(v);
    let mut tx = c.core.db.begin().await?;
    media_jobs::lock_shop(&mut tx, &t).await?;
    let p = media_jobs::lock_product_facts(&mut tx, &t, product)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Product not found".into()))?;
    if input.revision != Some(p.revision) {
        return Err(conflict("Product changed; save or reload first"));
    }
    if request.mode == Mode::Optimize {
        let source = request
            .source
            .as_deref()
            .ok_or(bad("Source asset required"))?;
        if !media_jobs::is_product_image(&mut tx, &t, product, source).await? {
            return Err(bad(
                "Optimization needs an uploaded image belonging to this product",
            ));
        }
    }
    if !c.images.provider().await?.configured {
        return Err(bad("Image provider not configured"));
    }
    if media_jobs::pending(&mut tx, &t, Some(product)).await? > 0 {
        return Err(conflict("An image job is already running for this product"));
    }
    if media_jobs::pending(&mut tx, &t, None).await? >= 10 {
        return Err(bad("Maximum 10 pending image jobs per shop"));
    }
    let id = uid();
    let instruction = image_job::instruction(&p.name, &p.description, &request.prompt);
    media_jobs::insert(
        &mut tx,
        &t,
        &id,
        product,
        p.revision,
        json!({"mode":request.mode.name(),"sourceId":input.source_id,"prompt":instruction}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"state":"queued"}))
}

pub async fn list(c: &Catalog, h: &RequestContext, product: &str) -> Result<Value> {
    permit(h, "catalog.read")?;
    let t = merchant(h)?;
    Ok(json!({"jobs":media_jobs::recent(&c.core, &t, product).await?}))
}

pub async fn detail(c: &Catalog, h: &RequestContext, id: &str) -> Result<Value> {
    permit(h, "catalog.read")?;
    let t = merchant(h)?;
    let job = media_jobs::outcome(&c.core, &t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Image job not found".into()))?;
    let mut v = json!({"id":id,"state":job.state,"error":job.error,"assetId":job.asset_id});
    if let Some(asset) = job.asset_id {
        let bytes = media_jobs::asset_content(&c.core, &t, &asset).await?;
        let preview = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        merge(&mut v, json!({ "preview": preview }))?;
    }
    Ok(v)
}

/// Publishes a reviewed image, only while the product is still at the revision the image was made for.
pub async fn apply(c: &Catalog, h: &RequestContext, id: &str, v: &Value) -> Result<Value> {
    permit(h, "catalog")?;
    let t = merchant(h)?;
    let mut tx = c.core.db.begin().await?;
    let job = media_jobs::lock(&mut tx, &t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Image job not found".into()))?;
    let Some(asset) = job.asset_id.filter(|_| job.state == "ready") else {
        return Err(conflict("Image is not ready for review"));
    };
    let revision = media_jobs::lock_product_revision(&mut tx, &t, &job.product_id).await?;
    if view::<ImageDecision>(v).revision != Some(revision) || revision != job.product_revision {
        return Err(conflict(
            "Product changed since image generation; start a fresh draft",
        ));
    }
    media_jobs::publish(&mut tx, &t, id, &asset).await?;
    tx.commit().await?;
    Ok(json!({"id":asset,"url":format!("/store-api/assets/{asset}?shop={t}"),"view":"","alt":{}}))
}

/// Runs at most one queued job; `false` means there was nothing to do.
pub async fn once(c: &Catalog) -> Result<bool> {
    media_jobs::fail_interrupted(&c.core).await?;
    if !c.images.provider().await?.configured {
        return Ok(false);
    }
    let Some(job) = media_jobs::claim(&c.core).await? else {
        return Ok(false);
    };
    match generate(c, &job.tenant, &job.product_id, &job.request).await {
        Ok(asset) => media_jobs::complete(&c.core, &job.tenant, &job.id, &asset).await?,
        Err(e) => media_jobs::fail(&c.core, &job.tenant, &job.id, &e.1).await?,
    }
    Ok(true)
}

async fn generate(c: &Catalog, t: &str, product: &str, request: &Value) -> Result<String> {
    let request: StoredImageJob = view(request);
    let source = if request.mode.as_deref() == Some("optimize") {
        Some(
            media_jobs::source(
                &c.core,
                t,
                product,
                request.source_id.as_deref().unwrap_or(""),
            )
            .await?,
        )
    } else {
        None
    };
    let prompt = request
        .prompt
        .as_deref()
        .ok_or(bad("Invalid image instructions"))?;
    let bytes = c.images.create(t, prompt, source).await?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let asset = uid();
    let mut tx = c.core.db.begin().await?;
    if media_jobs::lock_asset_count(&mut tx, t, product).await? >= 50 {
        return Err(bad("Maximum 50 assets per product"));
    }
    media_jobs::insert_generated(&mut tx, t, &asset, product, bytes, &digest).await?;
    tx.commit().await?;
    Ok(asset)
}
