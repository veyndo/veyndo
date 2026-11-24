//! Public append-only intake uses declared schemas, existing cart admission, quotas, records and outbox.
use super::{Entity, Manifest, data, site_forms};
use crate::cart_storage::load_cart;
use crate::foundation::App;
use crate::marketing;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;
use veyndo_app_platform::store::app_host_reads;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::hash;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
use veyndo_kernel::verified_kernel;
pub(super) async fn submit(
    a: &App,
    h: &RequestContext,
    m: &Manifest,
    e: &Entity,
    v: &Value,
) -> Result<Value> {
    let t = tenant(h)?;
    let live = veyndo_core::shop_scope::parent(a, &t).await?.is_none();
    let cart = load_cart(a, h).await?;
    if !verified_kernel::app_submission_admissible(
        !e.public_read,
        live,
        cart.tenant == t && cart.data.sales_channel == marketing::channel_id(h),
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Submissions require a private live inbox and matching cart".into(),
        ));
    }
    let intake_key = hash(&cart.id);
    let id = format!("submission_{}", uuid::Uuid::new_v4().simple());
    let mut tx = a.db.begin().await?;
    let kind = format!("app.{}.submitted", m.id);
    let (count, personal) =
        app_host_reads::submission_window(&mut tx, &t, &kind, &intake_key).await?;
    if count >= 60 || personal >= 5 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Please wait a minute before submitting another request".into(),
        ));
    }
    if m.id == "site_forms" {
        site_forms::admit(&mut tx, &t, marketing::channel_id(h), v).await?;
    }
    let _ = data::save_tx(
        &mut tx,
        &t,
        m,
        e,
        &json!({"id":id,"revision":0,"fields":v["fields"]}),
    )
    .await?;
    app_host_reads::submitted(
        &mut tx,
        &t,
        &kind,
        &json!({"app":m.id,"entity":e.name,"id":id,"intakeKey":intake_key}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"accepted":true}))
}
