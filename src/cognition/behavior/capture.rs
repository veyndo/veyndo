//! Cart-locked writes serialize with withdrawal; customer association is derived from authenticated native identity.
use super::{input::Event, *};
use veyndo_intelligence::store::behavior as store;
pub(super) async fn capture(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Event>,
) -> Result<Json<Value>> {
    v.validate()?;
    let c = load_cart(&a, &h).await?;
    if header(&h, "x-rac-channel-preview").is_some()
        || staging::parent(&a, &c.tenant).await?.is_some()
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sandbox and preview contexts do not record behavioral analytics".into(),
        ));
    }
    let customer = if header(&h, "x-customer-token").is_some() {
        let (_, email) = accounts::identity(&a, &h).await?;
        sqlx::query_scalar::<_, String>("SELECT id FROM customers WHERE tenant=$1 AND email=$2")
            .bind(&c.tenant)
            .bind(email)
            .fetch_optional(&a.db)
            .await?
    } else {
        None
    };
    let locale = language_context(&a, &h).await?.0;
    if let Some(product) = &v.product_id {
        marketing::admit_product(&a, &h, product).await?;
    }
    let mut tx = a.db.begin().await?;
    // Same lock order as consent withdrawal: cart -> settings -> receipt.
    let row = sqlx::query("SELECT data,status FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&c.tenant)
        .bind(&c.id)
        .fetch_one(&mut *tx)
        .await?;
    if row.get::<String, _>("status") != "open" {
        return Err(conflict("Behavioral events require an open cart context"));
    }
    let current: cart_model::Cart =
        serde_json::from_value(row.get("data")).map_err(|e| bad(e.to_string()))?;
    legal::require_locked(&mut tx, &c, "analytics").await?;
    let (settings, _) = commerce::scoped_locked(&mut tx, &c.tenant, &c.data.sales_channel).await?;
    let policy = &settings.legal.behavior_analytics;
    if !policy.enabled {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Native behavioral analytics is disabled for this sales channel".into(),
        ));
    }
    if let Some(product) = &v.product_id
        && v.kind == "cart_add"
        && !current.items.iter().any(|i| i.id == *product)
    {
        return Err(bad("Cart-add observation requires an actual cart item"));
    }
    let dimensions = json!({"browser":input::browser(header(&h,"user-agent").unwrap_or("")),"viewport":v.viewport,"surface":v.surface,"referrerHost":v.referrer_host,"locale":locale,"country":current.checkout.as_ref().map(|s|s.country.clone()).unwrap_or("unknown".into()),"geoCountry":geography::country(&h),"countrySource":"cart-destination"});
    let stored = store::record(
        &mut tx,
        store::Observation {
            tenant: &c.tenant,
            cart: &c.id,
            channel: &c.data.sales_channel,
            id: &v.event_id,
            kind: &v.kind,
            product: v.product_id.as_deref(),
            customer,
            dimensions,
            events_per_hour: i64::from(policy.events_per_hour),
            retention_days: policy.retention_days as i32,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(stored))
}
pub(super) async fn forget(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let n = store::forget(&a, &c.tenant, &c.id).await?;
    Ok(Json(json!({"deleted":n})))
}
pub(super) async fn export(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    Ok(Json(store::export(&a, &c.tenant, &c.id).await?))
}
