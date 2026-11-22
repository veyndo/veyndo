//! Company basis transport: reading and saving the shop-wide or channel-specific company data under the settings rights.
use crate::store::company;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::Value;
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};

/// The company basis of the shop, or of one channel.
pub async fn read(a: &State, h: &RequestContext, channel: Option<&str>) -> Result<Value> {
    let t = merchant(h)?;
    company::read(a, h, &t, channel).await
}

/// Saves the company basis of the shop, or of one channel, under its revision.
pub async fn save(a: &State, h: &RequestContext, v: Value, channel: Option<&str>) -> Result<Value> {
    let t = merchant(h)?;
    company::save(a, h, &t, v, channel).await
}

/// Reads the shop-wide company basis.
pub async fn get(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    merchant(&h)?;
    permit(&h, "settings.read")?;
    Ok(Json(read(&a, &h, None).await?))
}

/// Reads the company basis of one channel.
pub async fn get_channel(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(channel): Path<String>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    permit(&h, "settings.read")?;
    Ok(Json(read(&a, &h, Some(&channel)).await?))
}

/// Saves the shop-wide company basis.
pub async fn put(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    permit(&h, "settings.write")?;
    Ok(Json(save(&a, &h, v, None).await?))
}

/// Saves the company basis of one channel.
pub async fn put_channel(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(channel): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    permit(&h, "settings.write")?;
    Ok(Json(save(&a, &h, v, Some(&channel)).await?))
}
