//! The merchant product list: catalogue filters from the catalog component, app filters and extensions from the app host, and the opt-in preparation projection.
use crate::domain::views::Identified;
use axum::Json;
use axum::extract::{Query, State as Shared};
use serde_json::{Value, json};
use veyndo_app_model::Object;
use veyndo_app_platform::store::extension_filter::{admit as admit_filters, predicate};
use veyndo_app_platform::store::extension_projection::attach;
use veyndo_catalog::domain::preparation::{Source, filled_locales};
use veyndo_catalog::ports::Catalog;
use veyndo_catalog::service::catalog_reads::localize;
use veyndo_catalog::store::{media_jobs, product_list, translations};
use veyndo_catalog::types::product_list::ProductCriteria;
use veyndo_core::document::{merge, view};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::{config, languages};

/// The catalogue bounds plus the admission of app-contributed filters.
pub fn validate_criteria(c: &ProductCriteria) -> Result<i64> {
    let limit = c.validate()?;
    veyndo_app_platform::domain::extension_filter::decode(&c.app_filter)?;
    Ok(limit)
}
/// One page of the merchant product list with app filters, app extensions and the optional preparation projection.
pub async fn list_products(
    Shared(c0): Shared<Catalog>,
    h: RequestContext,
    Query(c): Query<ProductCriteria>,
) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let a = &c0.core;
    let t = merchant(&h)?;
    let limit = validate_criteria(&c)?;
    let app_filters = admit_filters(a, &h, Object::Product, &c.app_filter).await?;
    let (_, chain) = language_context(a, &h).await?;
    let mut listed = product_list::page(a, &t, &c, limit, &chain, |q| {
        predicate(q, Object::Product, &t, &app_filters)
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
    let translated = localize(
        &c0,
        &t,
        &chain,
        listed.iter().map(|l| l.product.clone()).collect(),
    )
    .await?;
    let mut elements = vec![];
    for (l, p) in listed.iter().zip(translated.iter()) {
        let mut v = json!(p);
        merge(&mut v, json!({ "active": l.active }))?;
        merge(&mut v, json!({ "productNumber": l.product_number }))?;
        merge(&mut v, json!({ "variantCount": l.variant_count }))?;
        merge(&mut v, json!({ "categoryIds": l.category_ids }))?;
        elements.push(v);
    }
    attach(a, &h, Object::Product, &mut elements).await?;
    if c.preparation {
        preparation(a, &t, &mut elements, &sources).await?;
    }
    let cursor = has_more.then(|| elements.last().unwrap()["id"].clone());
    Ok(Json(
        json!({"elements":elements,"hasMore":has_more,"nextCursor":cursor,"limit":limit}),
    ))
}
/// Attaches each product's filled languages and latest image job.
async fn preparation(
    a: &State,
    tenant: &str,
    products: &mut [Value],
    sources: &[(String, String, String)],
) -> Result<()> {
    if products.is_empty() {
        return Ok(());
    }
    let ids = products
        .iter()
        .filter_map(|p| view::<Identified>(p).id)
        .collect::<Vec<_>>();
    // Fetch only the already-authorized page, never the whole shop or binary media.
    let texts = translations::all_for_products(a, tenant, &ids).await?;
    let jobs = media_jobs::latest(a, tenant, &ids).await?;
    let languages = languages(a).await?;
    let (settings, _) = config(a, tenant).await?;
    let language_of = |locale: &str| {
        languages
            .iter()
            .find(|l| l.locale == locale)
            .map(|l| l.id.clone())
    };
    for product in products {
        let id = view::<Identified>(product).id.unwrap_or_default();
        let source = sources
            .iter()
            .find(|(product, _, _)| *product == id)
            .map(|(_, name, description)| Source { name, description });
        let locales = |description: bool| {
            filled_locales(
                &id,
                source,
                &texts,
                language_of,
                settings.languages(),
                description,
            )
        };
        let job = jobs.iter().find(|job| job.product_id == id);
        merge(
            product,
            json!({ "preparation": {
            "namedLocales":locales(false),"describedLocales":locales(true),
            "locales":settings.locales,"mainLocale":settings.main_locale,"imageJob":job
        } }),
        )?;
    }
    Ok(())
}
