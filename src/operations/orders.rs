//! Bounded order search, token-redacted detail and append-only operational notes.
use super::*;
pub(super) async fn list(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(c): axum::extract::Query<Criteria>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "orders.read")?;
    auth::permit(&h, "customers.pii")?;
    let n = super::list_criteria::validate(&c)?;
    let filters =
        apps::extension_filter::admit(&a, &h, apps::core_extensions::Object::Order, &c.app_filter)
            .await?;
    let rows = order_list::rows(&a, &t, &c, n, &filters).await?;
    let more = rows.len() > n as usize;
    let mut elements = rows
        .iter()
        .take(n as usize)
        .map(|r| {
            let mut o: Value = r.get("data");
            commerce::order_fields(&mut o);
            o["createdAt"] = json!(r.get::<String, _>("created"));
            o["customerEmail"] = json!(r.get::<Option<String>, _>("email"));
            o
        })
        .collect::<Vec<_>>();
    apps::core_extensions::attach(&a, &h, apps::core_extensions::Object::Order, &mut elements)
        .await?;
    let after = elements.last().map(|v| v["id"].clone());
    Ok(Json(
        json!({"elements":elements,"hasMore":more,"nextCursor":if more{after}else{None}}),
    ))
}
pub(super) async fn detail(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "orders.read")?;
    auth::permit(&h, "customers.pii")?;
    let mut v = veyndo_orders::store::order_reads::detail(&a, &t, &id).await?;
    let (machine, revision) = commerce::machine(&a.db, &t).await?;
    let (machine, revision) = apps::native_lifecycle::order_machine(&v, machine, revision)?;
    v["workflow"] = commerce::workflow(&v, &machine, revision, &h);
    apps::core_extensions::attach(
        &a,
        &h,
        apps::core_extensions::Object::Order,
        std::slice::from_mut(&mut v),
    )
    .await?;
    Ok(Json(v))
}
pub(super) async fn note(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "orders.write")?;
    auth::permit(&h, "customers.pii")?;
    veyndo_orders::store::order_reads::note(&a, &h, &t, &id, &v).await?;
    Ok(Json(json!({"saved":true})))
}
