//! Imported promotions use the ordinary revisioned owner and retain immutable provenance until a merchant reviews activation.
use super::*;
pub(crate) use veyndo_marketing::types::promotion::preserve_import;
pub(crate) async fn import_promotion(
    a: &App,
    h: &RequestContext,
    id: &str,
    data: &Value,
) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "automation.manage")?;
    if veyndo_core::shop_scope::parent(a, &t).await?.is_some() {
        return Err(bad(
            "Coupon migration needs its live workspace; activation remains separate",
        ));
    }
    if data["active"] != false || data["sourceImport"]["activationReviewed"] != false {
        return Err(bad("Imported coupons must start inactive and unreviewed"));
    }
    let previous: Option<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_promotions WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?;
    if let Some(previous) = previous {
        let incoming = &data["sourceImport"];
        let stored = &previous["sourceImport"];
        preserve_import(&previous, data)?;
        if stored.is_null() || stored["recordDigest"] != incoming["recordDigest"] {
            return Err(conflict(
                "Imported coupon source changed; review its conflict",
            ));
        }
        return Ok(json!({"id":id,"replayed":true}));
    }
    let _ = routes::save(
        State(a.clone()),
        h.clone(),
        Path(("promotions".into(), id.into())),
        Json(json!({"data":data,"revision":0})),
    )
    .await?;
    Ok(json!({"id":id,"replayed":false,"active":false}))
}
