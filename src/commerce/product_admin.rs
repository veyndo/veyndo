//! The merchant product list: catalogue filters from the catalog component, app filters and extensions from the app host.
use super::*;
pub(crate) use veyndo_catalog::types::product_list::ProductCriteria;

/// The catalogue bounds plus the admission of app-contributed filters.
pub(crate) fn validate_criteria(c: &ProductCriteria) -> Result<i64> {
    let limit = c.validate()?;
    apps::extension_filter::decode(&c.app_filter)?;
    Ok(limit)
}
pub(crate) fn product_admin_router() -> Router<App> {
    Router::new().secure_route(
        "/api/merchant/products",
        &[("GET", "catalog.read"), ("POST", "catalog.write")],
        get(list_products).post(super::create_product),
    )
}
pub(crate) async fn list_products(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(c): axum::extract::Query<ProductCriteria>,
) -> Result<Json<Value>> {
    auth::permit(&h, "catalog.read")?;
    let t = merchant(&a, &h)?;
    let limit = validate_criteria(&c)?;
    let app_filters = apps::extension_filter::admit(
        &a,
        &h,
        apps::core_extensions::Object::Product,
        &c.app_filter,
    )
    .await?;
    let (_, chain) = language_context(&a, &h).await?;
    let mut listed =
        veyndo_catalog::store::product_list::page(&a.core, &t, &c, limit, &chain, |q| {
            apps::extension_filter::predicate(
                q,
                apps::core_extensions::Object::Product,
                &t,
                &app_filters,
            )
        })
        .await?;
    let has_more = listed.len() > limit as usize;
    listed.truncate(limit as usize);
    let sources = listed
        .iter()
        .map(|l| {
            (
                l.product.id.clone(),
                l.product.name.clone(),
                l.product.description.clone(),
            )
        })
        .collect::<Vec<_>>();
    let translated = localize_products(
        &a,
        &t,
        &chain,
        listed.iter().map(|l| l.product.clone()).collect(),
    )
    .await?;
    let mut elements = vec![];
    for (l, p) in listed.iter().zip(translated.iter()) {
        let mut v = json!(p);
        v["active"] = json!(l.active);
        v["productNumber"] = json!(l.product_number);
        v["variantCount"] = json!(l.variant_count);
        v["categoryIds"] = json!(l.category_ids);
        elements.push(v);
    }
    apps::core_extensions::attach(
        &a,
        &h,
        apps::core_extensions::Object::Product,
        &mut elements,
    )
    .await?;
    if c.preparation {
        super::product_preparation::attach(&a, &t, &mut elements, &sources).await?;
    }
    let cursor = has_more.then(|| elements.last().unwrap()["id"].clone());
    Ok(Json(
        json!({"elements":elements,"hasMore":has_more,"nextCursor":cursor,"limit":limit}),
    ))
}
